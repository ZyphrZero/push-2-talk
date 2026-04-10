import type { HistoryRecord } from '../types';
import { HISTORY_KEY, MAX_HISTORY } from '../constants';

export const loadHistory = (): HistoryRecord[] => {
  try {
    const data = localStorage.getItem(HISTORY_KEY);
    return data ? JSON.parse(data) : [];
  } catch {
    return [];
  }
};

export const saveHistory = (records: HistoryRecord[]): void => {
  try {
    localStorage.setItem(HISTORY_KEY, JSON.stringify(records.slice(0, MAX_HISTORY)));
  } catch {
    // QuotaExceededError: 截断 selectedText 后重试
    const trimmed = records.slice(0, MAX_HISTORY).map((r) => ({
      ...r,
      selectedText: r.selectedText ? r.selectedText.slice(0, 200) + "…" : r.selectedText,
    }));
    try {
      localStorage.setItem(HISTORY_KEY, JSON.stringify(trimmed));
    } catch {
      // 仍然失败则清空最旧的一半记录
      const half = trimmed.slice(0, Math.ceil(trimmed.length / 2));
      localStorage.setItem(HISTORY_KEY, JSON.stringify(half));
    }
  }
};

export const addHistoryRecord = (
  records: HistoryRecord[],
  record: HistoryRecord
): HistoryRecord[] => {
  const updated = [record, ...records].slice(0, MAX_HISTORY);
  saveHistory(updated);
  return updated;
};

export const clearHistory = (): void => {
  localStorage.setItem(HISTORY_KEY, JSON.stringify([]));
};
