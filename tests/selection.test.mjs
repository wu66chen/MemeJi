import { test } from 'node:test';
import assert from 'node:assert/strict';
import { selectIds } from '../src/lib/selection.ts';

test('plain click replaces selection; Ctrl toggles without duplicates', () => {
  assert.deepEqual(selectIds([1,2,3], [1,2], 1, 3, {range:false,toggle:false}), {ids:[3],anchor:3});
  assert.deepEqual(selectIds([1,2,3], [1,2], 1, 2, {range:false,toggle:true}).ids, [1]);
  assert.deepEqual(selectIds([1,2,3], [1], 1, 3, {range:false,toggle:true}).ids, [1,3]);
});
test('Shift selects a range in visible order, including backwards and additive ranges', () => {
  assert.deepEqual(selectIds([7,3,9,2], [2], 2, 3, {range:true,toggle:false}), {ids:[3,9,2],anchor:2});
  assert.deepEqual(selectIds([7,3,9,2], [7,9], 9, 2, {range:true,toggle:true}).ids, [7,9,2]);
});
test('filtered-out anchors and selections cannot act on hidden items', () => {
  assert.deepEqual(selectIds([4,5], [1], 1, 5, {range:true,toggle:false}), {ids:[5],anchor:5});
  assert.deepEqual(selectIds([4,5], [1,4], 1, 5, {range:false,toggle:true}).ids, [4,5]);
});
