import test from 'node:test';
import assert from 'node:assert/strict';
import { calculate } from '../src/calculator.js';
test('precedence, right associative powers, signs and percentage', () => { assert.equal(calculate('2+3*4'),14); assert.equal(calculate('(2+3)*4'),20); assert.equal(calculate('2^3^2'),512); assert.equal(calculate('-2^2'),-4); assert.equal(calculate('200 * 10%'),20); assert.equal(calculate('1e2 / 4'),25); });
test('rejects code, unsupported tokens, non-finite results and hostile depth', () => { for (const value of ['globalThis.alert(1)','2;3','0/0','1/0','2**3','Infinity','9^9999','('.repeat(40)+'1'+')'.repeat(40), '-'.repeat(100)+'1','1'.repeat(513),'2 +']) assert.throws(() => calculate(value)); });
