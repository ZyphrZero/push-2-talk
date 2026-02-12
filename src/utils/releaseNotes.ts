/**
 * 累积 release notes 获取工具
 * 当用户跳过多个版本时，合并展示所有中间版本的更新内容
 */

const GITHUB_API = "https://api.github.com/repos/yyyzl/push-2-talk/releases";

/** 简单 semver 比较: 返回 -1 / 0 / 1 */
function compareVersions(a: string, b: string): number {
  const pa = a.replace(/^v/, "").split(".").map(Number);
  const pb = b.replace(/^v/, "").split(".").map(Number);
  for (let i = 0; i < 3; i++) {
    const va = pa[i] ?? 0;
    const vb = pb[i] ?? 0;
    if (va < vb) return -1;
    if (va > vb) return 1;
  }
  return 0;
}

interface GithubRelease {
  tag_name: string;
  body: string | null;
  prerelease: boolean;
  draft: boolean;
}

/**
 * 获取 currentVersion 到 latestVersion 之间所有版本的 release notes（含 latestVersion）。
 * 失败时返回 null（调用方应 fallback 到 latest.json 中的 notes）。
 */
export async function fetchAccumulatedNotes(
  currentVersion: string,
  latestVersion: string,
): Promise<string | null> {
  // 版本相同或只差一个版本时没必要请求
  if (compareVersions(currentVersion, latestVersion) >= 0) return null;

  try {
    const resp = await fetch(GITHUB_API + "?per_page=50", {
      headers: { Accept: "application/vnd.github+json" },
      signal: AbortSignal.timeout(8000),
    });
    if (!resp.ok) return null;

    const releases: GithubRelease[] = await resp.json();

    // 筛选: currentVersion < tag <= latestVersion, 排除 prerelease / draft
    const relevant = releases
      .filter((r) => {
        if (r.prerelease || r.draft) return false;
        const cmp1 = compareVersions(r.tag_name, currentVersion);
        const cmp2 = compareVersions(r.tag_name, latestVersion);
        return cmp1 > 0 && cmp2 <= 0;
      })
      .sort((a, b) => compareVersions(b.tag_name, a.tag_name)); // 新版本在前

    if (relevant.length <= 1) return null; // 只有一个版本，用原始 notes 即可

    // 按分类合并所有版本的条目（去重同类标题）
    // CI 生成的格式: "## ✨ 新功能\n- xxx\n\n## 🐛 Bug 修复\n- yyy"
    const categoryOrder = ["✨ 新功能", "🐛 Bug 修复", "🚀 优化改进", "📦 其他"];
    const merged = new Map<string, string[]>();

    for (const r of relevant) {
      const body = (r.body || "").trim();
      if (!body) continue;

      // 按 ## 标题拆分段落
      const sections = body.split(/^## /m).filter(Boolean);
      for (const section of sections) {
        const newlineIdx = section.indexOf("\n");
        if (newlineIdx === -1) continue;
        const heading = section.slice(0, newlineIdx).trim();
        const items = section
          .slice(newlineIdx + 1)
          .trim()
          .split("\n")
          .filter((l) => l.startsWith("- "));
        if (items.length === 0) continue;

        const existing = merged.get(heading) ?? [];
        // 去重：相同文本不重复添加
        for (const item of items) {
          if (!existing.includes(item)) existing.push(item);
        }
        merged.set(heading, existing);
      }
    }

    // 按固定顺序输出
    const parts: string[] = [];
    for (const cat of categoryOrder) {
      const items = merged.get(cat);
      if (items?.length) {
        parts.push(`## ${cat}\n${items.join("\n")}`);
        merged.delete(cat);
      }
    }
    // 剩余未知分类
    for (const [heading, items] of merged) {
      if (items.length) parts.push(`## ${heading}\n${items.join("\n")}`);
    }

    return parts.length > 0 ? parts.join("\n\n") : null;
  } catch (err) {
    console.warn("获取累积 release notes 失败, 将使用最新版本的 notes:", err);
    return null;
  }
}
