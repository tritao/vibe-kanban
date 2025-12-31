import { useCallback, useMemo } from 'react';
import type { Diff, PatchType } from 'shared/types';
import { useJsonPatchWsStream } from './useJsonPatchWsStream';

interface DiffEntries {
  [filePath: string]: PatchType;
}

type DiffStreamEvent = {
  entries: DiffEntries;
};

export interface UseDiffStreamOptions {
  statsOnly?: boolean;
  /**
   * Forces the diff WebSocket stream to restart when this changes.
   * Useful when the "base commit" changes (e.g. after rebase / target branch
   * change), since the server-side diff stream is computed against the base at
   * stream creation time.
   */
  refreshKey?: string | number;
}

interface UseDiffStreamResult {
  diffs: Diff[];
  error: string | null;
}

export const useDiffStream = (
  attemptId: string | null,
  enabled: boolean,
  options?: UseDiffStreamOptions
): UseDiffStreamResult => {
  const endpoint = (() => {
    if (!attemptId) return undefined;
    const query = `/api/task-attempts/${attemptId}/diff/ws`;
    const params = new URLSearchParams();

    if (typeof options?.statsOnly === 'boolean') {
      params.set('stats_only', String(options.statsOnly));
    }

    if (options?.refreshKey != null) {
      params.set('rev', String(options.refreshKey));
    }

    const suffix = params.toString();
    return suffix ? `${query}?${suffix}` : query;
  })();

  const initialData = useCallback(
    (): DiffStreamEvent => ({
      entries: {},
    }),
    []
  );

  const { data, error } = useJsonPatchWsStream<DiffStreamEvent>(
    endpoint,
    enabled && !!attemptId,
    initialData
    // No need for injectInitialEntry or deduplicatePatches for diffs
  );

  const diffs = useMemo(() => {
    return Object.values(data?.entries ?? {})
      .filter((entry) => entry?.type === 'DIFF')
      .map((entry) => entry.content);
  }, [data?.entries]);

  return { diffs, error };
};
