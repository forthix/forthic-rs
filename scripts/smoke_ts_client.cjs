#!/usr/bin/env node
/**
 * Cross-runtime smoke: drive the forthic-rs JSON-RPC server with the real
 * forthic-ts JsonRpcClient. The compatibility proof for the wire format.
 *
 * Usage: node smoke_ts_client.cjs <port> <forthic-ts-dir>
 * Requires a built forthic-ts checkout (dist/cjs present).
 */
const path = require('path');

const port = process.argv[2];
const tsDir = process.argv[3];
if (!port || !tsDir) {
  console.error('usage: smoke_ts_client.cjs <port> <forthic-ts-dir>');
  process.exit(2);
}

const { Temporal } = require(path.join(tsDir, 'node_modules', 'temporal-polyfill'));
globalThis.Temporal = globalThis.Temporal ?? Temporal;
const { JsonRpcClient } = require(path.join(tsDir, 'dist', 'cjs', 'jsonrpc', 'client.js'));

function assert(cond, message) {
  if (!cond) {
    console.error(`SMOKE FAILED: ${message}`);
    process.exit(1);
  }
}

(async () => {
  const client = new JsonRpcClient(`127.0.0.1:${port}`);

  // 1. Mixed-type stack round-trips through the rust runtime
  const zoned = Temporal.ZonedDateTime.from('2020-06-05T10:15:00-07:00[America/Los_Angeles]');
  const stack = [
    42, 'hello', 3.25, true, null,
    [1, [2, { deep: 'record' }]],
    Temporal.PlainDate.from('2020-06-05'),
    Temporal.PlainTime.from('09:30:00'),
    zoned,
  ];
  const result = await client.executeWord('DUP', stack);
  assert(result.length === stack.length + 1, `DUP: expected ${stack.length + 1} items, got ${result.length}`);
  assert(result[0] === 42 && result[1] === 'hello' && result[2] === 3.25, 'scalars survived');
  assert(result[3] === true && result[4] === null, 'bool/null survived');
  assert(JSON.stringify(result[5]) === JSON.stringify(stack[5]), 'nested containers survived');
  assert(result[6] instanceof Temporal.PlainDate && result[6].equals(stack[6]), 'PlainDate survived');
  assert(result[7] instanceof Temporal.PlainTime && result[7].equals(stack[7]), 'PlainTime survived');
  assert(result[8] instanceof Temporal.ZonedDateTime && result[8].equals(zoned), 'ZonedDateTime survived');
  assert(result[9] instanceof Temporal.ZonedDateTime && result[9].equals(zoned), 'DUP duplicated the zoned datetime');

  // 2. executeSequence
  const seq = await client.executeSequence(['DUP', '+'], [21]);
  assert(seq.length === 1 && seq[0] === 42, `executeSequence: expected [42], got ${JSON.stringify(seq)}`);

  // 3. listModules
  const modules = await client.listModules();
  assert(Array.isArray(modules), 'listModules returns an array');

  // 4. Rich errors surface as RemoteRuntimeError with rust metadata
  let threw = false;
  try {
    await client.executeWord('NO-SUCH-WORD', []);
  } catch (e) {
    threw = true;
    assert(e.constructor.name === 'RemoteRuntimeError', `error class: ${e.constructor.name}`);
    assert(e.runtime === 'rust', `error runtime: ${e.runtime}`);
    assert(e.errorType === 'UnknownWord', `error type: ${e.errorType}`);
    assert(e.context && e.context.word_name === 'NO-SUCH-WORD', 'error context intact');
  }
  assert(threw, 'unknown word raised');

  console.log('cross-runtime smoke OK (ts client <-> rs server)');
})().catch((e) => {
  console.error('SMOKE FAILED:', e.message);
  process.exit(1);
});
