import { test } from 'node:test';
import assert from 'node:assert/strict';
import { getDetailPanelState, shouldShowDetailsFromMenu } from '../src/lib/detail-panel-state.ts';

test('detail column remains visible as an empty placeholder with no single selection', () => {
  assert.deepEqual(getDetailPanelState(true, 0), {
    panelVisible: true,
    hasSelectedMeme: false,
    canShowDetailsFromMenu: false,
  });
  assert.deepEqual(getDetailPanelState(true, 3), {
    panelVisible: true,
    hasSelectedMeme: false,
    canShowDetailsFromMenu: false,
  });
});

test('header toggle controls visibility and context action only opens a single meme', () => {
  assert.deepEqual(getDetailPanelState(true, 1), {
    panelVisible: true,
    hasSelectedMeme: true,
    canShowDetailsFromMenu: false,
  });
  assert.deepEqual(getDetailPanelState(false, 1), {
    panelVisible: false,
    hasSelectedMeme: true,
    canShowDetailsFromMenu: true,
  });
  assert.equal(getDetailPanelState(false, 0).canShowDetailsFromMenu, false);
  assert.equal(getDetailPanelState(false, 2).canShowDetailsFromMenu, false);
});

test('context menu detail action enables only while closed with exactly one selected meme', () => {
  assert.equal(shouldShowDetailsFromMenu(true, 1), false);
  assert.equal(shouldShowDetailsFromMenu(false, 0), false);
  assert.equal(shouldShowDetailsFromMenu(false, 1), true);
  assert.equal(shouldShowDetailsFromMenu(false, 2), false);
});
