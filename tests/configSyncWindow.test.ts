import assert from "node:assert/strict";
import test from "node:test";
import {
  createConfigSyncWindowController,
  getSyncWindowNoticeMessage,
  scheduleSyncWindowRelease,
} from "../src/utils/configSyncWindow";

test("begin 会打开同步窗口并返回递增 token", () => {
  const controller = createConfigSyncWindowController();

  assert.equal(controller.isSuppressed(), false);

  const firstToken = controller.begin("initial_load");
  assert.equal(firstToken, 1);
  assert.equal(controller.isSuppressed(), true);
  assert.equal(controller.currentSource(), "initial_load");

  const secondToken = controller.begin("external_config_updated");
  assert.equal(secondToken, 2);
  assert.equal(controller.isSuppressed(), true);
  assert.equal(controller.currentSource(), "external_config_updated");
});

test("旧 token 不会提前结束新一轮同步窗口", () => {
  const controller = createConfigSyncWindowController();

  const oldToken = controller.begin("external_config_updated");
  const newToken = controller.begin("external_config_updated");

  assert.equal(controller.complete(oldToken), false);
  assert.equal(controller.isSuppressed(), true);

  assert.equal(controller.complete(newToken), true);
  assert.equal(controller.isSuppressed(), false);
  assert.equal(controller.currentSource(), null);
});

test("snapshot 会给出 UI 所需的同步窗口状态", () => {
  const controller = createConfigSyncWindowController();

  assert.deepEqual(controller.snapshot(), {
    isSuppressed: false,
    source: null,
    isExternalSyncing: false,
  });

  const token = controller.begin("external_config_updated");

  assert.deepEqual(controller.snapshot(), {
    isSuppressed: true,
    source: "external_config_updated",
    isExternalSyncing: true,
  });

  controller.complete(token);

  assert.deepEqual(controller.snapshot(), {
    isSuppressed: false,
    source: null,
    isExternalSyncing: false,
  });
});

test("scheduleSyncWindowRelease 会等待微任务与双 rAF 后释放", () => {
  const microtasks: Array<() => void> = [];
  const frameCallbacks: Array<() => void> = [];
  const releasedTokens: number[] = [];

  scheduleSyncWindowRelease({
    token: 7,
    complete: (token) => {
      releasedTokens.push(token);
    },
    queueMicrotaskFn: (callback) => {
      microtasks.push(callback);
    },
    requestAnimationFrameFn: (callback) => {
      frameCallbacks.push(callback);
      return frameCallbacks.length;
    },
  });

  assert.deepEqual(releasedTokens, []);
  assert.equal(microtasks.length, 1);

  microtasks.shift()?.();
  assert.equal(frameCallbacks.length, 1);
  frameCallbacks.shift()?.();
  assert.equal(frameCallbacks.length, 1);
  frameCallbacks.shift()?.();

  assert.deepEqual(releasedTokens, [7]);
});

test("全局同步提示文案会基于 source 区分初始加载与外部更新", () => {
  assert.equal(getSyncWindowNoticeMessage("initial_load"), "正在加载初始配置");
  assert.equal(getSyncWindowNoticeMessage("external_config_updated"), "正在同步外部配置");
  assert.equal(getSyncWindowNoticeMessage(null), null);
});
