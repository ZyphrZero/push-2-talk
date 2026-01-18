import { BookOpen, Key, FileClock, Github, ExternalLink, HelpCircle } from "lucide-react";
import { EXTERNAL_LINKS } from "../constants";
import { openUrl } from "@tauri-apps/plugin-opener";

export function HelpPage() {
  return (
    <div className="mx-auto max-w-3xl space-y-6 font-sans">
      <div className="bg-white border border-[var(--stone)] rounded-2xl p-6 space-y-5">
        <div className="flex items-center gap-2 text-xs font-bold text-stone-500 uppercase tracking-widest">
          <HelpCircle size={14} />
          <span>帮助与支持</span>
        </div>

        <div className="grid grid-cols-2 gap-4">
          <button
            onClick={() => openUrl(EXTERNAL_LINKS.tutorial)}
            className="flex items-center gap-3 p-4 bg-[var(--paper)] border border-[var(--stone)] rounded-2xl hover:border-[var(--sage)] hover:shadow-sm transition-all"
          >
            <div className="p-2 rounded-xl bg-[rgba(106,155,204,0.12)] text-[var(--steel)]">
              <BookOpen size={16} />
            </div>
            <div className="flex-1 text-left">
              <div className="text-sm font-bold text-[var(--ink)]">使用教程</div>
            </div>
            <ExternalLink size={14} className="text-stone-400" />
          </button>

          <button
            onClick={() => openUrl(EXTERNAL_LINKS.apiKeyGuide)}
            className="flex items-center gap-3 p-4 bg-[var(--paper)] border border-[var(--stone)] rounded-2xl hover:border-[var(--sage)] hover:shadow-sm transition-all"
          >
            <div className="p-2 rounded-xl bg-[rgba(120,140,93,0.12)] text-[var(--sage)]">
              <Key size={16} />
            </div>
            <div className="flex-1 text-left">
              <div className="text-sm font-bold text-[var(--ink)]">API Key 申请</div>
            </div>
            <ExternalLink size={14} className="text-stone-400" />
          </button>

          <button
            onClick={() => openUrl(EXTERNAL_LINKS.changelog)}
            className="flex items-center gap-3 p-4 bg-[var(--paper)] border border-[var(--stone)] rounded-2xl hover:border-[var(--sage)] hover:shadow-sm transition-all"
          >
            <div className="p-2 rounded-xl bg-[rgba(217,119,87,0.12)] text-[var(--crail)]">
              <FileClock size={16} />
            </div>
            <div className="flex-1 text-left">
              <div className="text-sm font-bold text-[var(--ink)]">更新日志</div>
            </div>
            <ExternalLink size={14} className="text-stone-400" />
          </button>

          <button
            onClick={() => openUrl(EXTERNAL_LINKS.github)}
            className="flex items-center gap-3 p-4 bg-[var(--paper)] border border-[var(--stone)] rounded-2xl hover:border-[var(--sage)] hover:shadow-sm transition-all"
          >
            <div className="p-2 rounded-xl bg-[rgba(20,20,19,0.12)] text-[var(--ink)]">
              <Github size={16} />
            </div>
            <div className="flex-1 text-left">
              <div className="text-sm font-bold text-[var(--ink)]">GitHub 仓库</div>
            </div>
            <ExternalLink size={14} className="text-stone-400" />
          </button>
        </div>
      </div>
    </div>
  );
}
