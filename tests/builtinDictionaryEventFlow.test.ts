import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";

test("useTauriEventListeners 应监听 builtin_dictionary_updated 事件", async () => {
  const source = await readFile("src/hooks/useTauriEventListeners.ts", "utf8");
  assert.match(source, /builtin_dictionary_updated/);
  assert.match(source, /fetchBuiltinDomains/);
  assert.match(source, /setBuiltinDomainsSnapshot/);
});

test("DictionaryPage 应通过版本号触发 reload，而不是重复监听事件", async () => {
  const source = await readFile("src/pages/DictionaryPage.tsx", "utf8");
  assert.doesNotMatch(source, /listen\("builtin_dictionary_updated"/);
  assert.match(source, /builtinDictionaryVersion/);
  assert.match(source, /\[reloadBuiltinDomains, builtinDictionaryVersion\]/);
});

test("App 应在内置词库更新后绕过 configHash 触发 applyRuntimeConfig", async () => {
  const appSource = await readFile("src/App.tsx", "utf8");
  assert.match(appSource, /builtin_dictionary_updated|onBuiltinDictionaryUpdated/);
  assert.match(appSource, /applyRuntimeConfig/);
});
