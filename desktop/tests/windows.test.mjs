import test from 'node:test';
import assert from 'node:assert/strict';
import { fit, openWindow, nextWindow } from '../src/shell/windows.js';
test('opening and restoring a minimized app preserves geometry and creates one instance', () => {
  let windows = openWindow([], 'Calculator', 1366, 900);
  assert.equal(windows[0].rect.width, 360); assert.equal(windows[0].rect.height, 480);
  windows[0].rect.x = 73; windows[0].minimized = true;
  windows = openWindow(windows, 'Files', 1366, 900);
  windows = openWindow(windows, 'Calculator', 1366, 900);
  assert.equal(windows.length, 2); assert.equal(windows[1].rect.x, 73);
  assert.equal(windows[1].minimized, false); assert.equal(nextWindow(windows), 'Calculator');
  windows[1].minimized = true; assert.equal(nextWindow(windows), 'Files');
  windows[0].minimized = true; assert.equal(nextWindow(windows), 'Home');
});
test('geometry remains within safe workspace after resizing or display changes', () => {
  for (const [width, height] of [[1920,1080],[1366,768],[390,844],[320,480]]) {
    const rect = fit({ x: 2000, y: -50, width: 960, height: 640 }, width, height);
    assert.ok(rect.x >= 16 && rect.y >= 56);
    assert.ok(rect.x + rect.width <= width - 16);
    assert.ok(rect.y + rect.height <= height - 88);
  }
});
test('closing the top window selects the most recent visible app', () => {
  let windows = openWindow([], 'Files', 1920, 1080);
  assert.equal(windows[0].rect.width, 960); assert.equal(windows[0].rect.height, 640);
  windows = openWindow(windows, 'Notes', 1920, 1080);
  windows = openWindow(windows, 'Photos', 1920, 1080);
  windows[1].minimized = true;
  assert.equal(nextWindow(windows.filter(w => w.app !== 'Photos')), 'Files');
  assert.equal(nextWindow([]), 'Home');
});
