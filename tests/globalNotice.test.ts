import assert from "node:assert/strict";
import test from "node:test";
import { resolveGlobalNotice } from "../src/utils/globalNotice";

test("GlobalNoticeBar 无障碍属性：role/status + aria-live/polite", async () => {
  const { readFile } = await import("node:fs/promises");
  const content = await readFile("src/components/common/GlobalNoticeBar.tsx", "utf8");

  assert.match(content, /role=\"status\"/);
  assert.match(content, /aria-live=\"polite\"/);
});

test("同步窗口提示优先级最高", () => {
  const notice = resolveGlobalNotice({
    syncWindowSource: "external_config_updated",
    syncStatus: "syncing",
    updateStatus: "downloading",
    updateDownloadProgress: 26,
  });

  assert.deepEqual(notice, {
    message: "正在同步外部配置",
    loading: true,
    tone: "info",
  });
});

test("保存状态次优先：saving/success/error", () => {
  assert.deepEqual(
    resolveGlobalNotice({
      syncWindowSource: null,
      syncStatus: "syncing",
      updateStatus: "checking",
    }),
    {
      message: "正在保存配置",
      loading: true,
      tone: "info",
    },
  );

  assert.deepEqual(
    resolveGlobalNotice({
      syncWindowSource: null,
      syncStatus: "success",
      updateStatus: "checking",
    }),
    {
      message: "配置已同步",
      loading: false,
      tone: "success",
    },
  );

  assert.deepEqual(
    resolveGlobalNotice({
      syncWindowSource: null,
      syncStatus: "error",
      updateStatus: "checking",
    }),
    {
      message: "配置保存失败，请稍后重试",
      loading: false,
      tone: "error",
    },
  );
});

test("更新状态：checking/downloading/ready/available", () => {
  assert.deepEqual(
    resolveGlobalNotice({
      syncWindowSource: null,
      syncStatus: "idle",
      updateStatus: "checking",
    }),
    {
      message: "正在检查更新",
      loading: true,
      tone: "info",
    },
  );

  assert.deepEqual(
    resolveGlobalNotice({
      syncWindowSource: null,
      syncStatus: "idle",
      updateStatus: "downloading",
      updateDownloadProgress: 32,
    }),
    {
      message: "正在下载更新 32%",
      loading: true,
      tone: "info",
    },
  );

  assert.deepEqual(
    resolveGlobalNotice({
      syncWindowSource: null,
      syncStatus: "idle",
      updateStatus: "ready",
    }),
    {
      message: "更新已就绪，正在重启应用",
      loading: false,
      tone: "success",
    },
  );

  assert.deepEqual(
    resolveGlobalNotice({
      syncWindowSource: null,
      syncStatus: "idle",
      updateStatus: "available",
    }),
    {
      message: "发现新版本，可前往偏好设置更新",
      loading: false,
      tone: "warning",
    },
  );
});

test("无状态时不显示全局提示", () => {
  assert.equal(
    resolveGlobalNotice({
      syncWindowSource: null,
      syncStatus: "idle",
      updateStatus: "idle",
    }),
    null,
  );
});
