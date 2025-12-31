import { useDiffStream } from '@/hooks/useDiffStream';
import { useMemo, useCallback, useState, useEffect, useRef } from 'react';
import { useTranslation } from 'react-i18next';
import { Loader } from '@/components/ui/loader';
import { Button } from '@/components/ui/button';
import DiffViewSwitch from '@/components/DiffViewSwitch';
import DiffCard from '@/components/DiffCard';
import { useDiffSummary } from '@/hooks/useDiffSummary';
import { NewCardHeader } from '@/components/ui/new-card';
import { ChevronsUp, ChevronsDown } from 'lucide-react';
import {
  Tooltip,
  TooltipContent,
  TooltipProvider,
  TooltipTrigger,
} from '@/components/ui/tooltip';
import type { Diff, DiffChangeKind } from 'shared/types';
import type { Workspace } from 'shared/types';
import GitOperations, {
  type GitOperationsInputs,
} from '@/components/tasks/Toolbar/GitOperations.tsx';

interface DiffsPanelProps {
  selectedAttempt: Workspace | null;
  gitOps?: GitOperationsInputs;
}

type DiffCollapseDefaults = Record<DiffChangeKind, boolean>;

const DEFAULT_DIFF_COLLAPSE_DEFAULTS: DiffCollapseDefaults = {
  added: false,
  deleted: true,
  modified: false,
  renamed: true,
  copied: true,
  permissionChange: true,
};

const DEFAULT_COLLAPSE_MAX_LINES = 200;

const exceedsMaxLineCount = (d: Diff, maxLines: number): boolean => {
  if (d.additions != null || d.deletions != null)
    return (d.additions ?? 0) + (d.deletions ?? 0) > maxLines;

  return true;
};

const getDiffId = ({ diff, index }: { diff: Diff; index: number }) =>
  `${diff.newPath || diff.oldPath || index}`;

export function DiffsPanel({ selectedAttempt, gitOps }: DiffsPanelProps) {
  const { t } = useTranslation('tasks');
  const [loadingState, setLoadingState] = useState<
    'loading' | 'loaded' | 'timed-out'
  >('loading');
  const [collapsedIds, setCollapsedIds] = useState<Set<string>>(new Set());
  const [processedIds, setProcessedIds] = useState<Set<string>>(new Set());
  const lastNonEmptyDiffsRef = useRef<Diff[]>([]);

  const diffStreamRefreshKey = useMemo(() => {
    if (!gitOps?.branchStatus?.length) return undefined;
    return gitOps.branchStatus
      .map(
        (s) =>
          `${s.repo_id}:${s.target_branch_name}:${s.head_oid ?? ''}`
      )
      .sort()
      .join('|');
  }, [gitOps?.branchStatus]);

  const { diffs, error } = useDiffStream(selectedAttempt?.id ?? null, true, {
    refreshKey: diffStreamRefreshKey,
  });
  const { fileCount, added, deleted } = useDiffSummary(
    selectedAttempt?.id ?? null,
    diffStreamRefreshKey
  );

  useEffect(() => {
    if (diffs.length > 0) {
      lastNonEmptyDiffsRef.current = diffs;
    }
  }, [diffs]);

  const renderDiffs = useMemo(() => {
    if (diffs.length > 0) return diffs;
    if (loadingState === 'loading') return lastNonEmptyDiffsRef.current;
    return diffs;
  }, [diffs, loadingState]);

  const renderSummary = useMemo(() => {
    if (
      loadingState === 'loading' &&
      renderDiffs.length > 0 &&
      fileCount === 0
    ) {
      return renderDiffs.reduce(
        (acc, d) => {
          acc.added += d.additions ?? 0;
          acc.deleted += d.deletions ?? 0;
          return acc;
        },
        { fileCount: renderDiffs.length, added: 0, deleted: 0 }
      );
    }

    return { fileCount, added, deleted };
  }, [added, deleted, fileCount, loadingState, renderDiffs]);

  // If no diffs arrive within 3 seconds, stop showing the spinner
  useEffect(() => {
    if (loadingState !== 'loading') return;
    const timer = setTimeout(() => setLoadingState('timed-out'), 3000);
    return () => clearTimeout(timer);
  }, [loadingState]);

  useEffect(() => {
    lastNonEmptyDiffsRef.current = [];
    setLoadingState('loading');
    setCollapsedIds(new Set());
    setProcessedIds(new Set());
  }, [selectedAttempt?.id]);

  // Mark loaded once the (new) stream yields diffs.
  useEffect(() => {
    if (diffs.length > 0 && loadingState === 'loading') {
      setLoadingState('loaded');
    }
  }, [diffs.length, loadingState]);

  // When we intentionally restart the diff stream (e.g. after rebase), avoid
  // clearing the UI immediately (flicker); the stream will reconnect and
  // repopulate diffs.
  useEffect(() => {
    setLoadingState('loading');
  }, [diffStreamRefreshKey]);

  if (diffs.length > 0) {
    const newDiffs = diffs
      .map((d, index) => ({ diff: d, index }))
      .filter((d) => {
        const id = getDiffId(d);
        return !processedIds.has(id);
      });

    if (newDiffs.length > 0) {
      const newIds = newDiffs.map(getDiffId);
      const toCollapse = newDiffs
        .filter(
          ({ diff }) =>
            DEFAULT_DIFF_COLLAPSE_DEFAULTS[diff.change] ||
            exceedsMaxLineCount(diff, DEFAULT_COLLAPSE_MAX_LINES)
        )
        .map(getDiffId);

      setProcessedIds((prev) => new Set([...prev, ...newIds]));
      if (toCollapse.length > 0) {
        setCollapsedIds((prev) => new Set([...prev, ...toCollapse]));
      }
    }
  }

  const loading = loadingState === 'loading' && renderDiffs.length === 0;

  const ids = useMemo(() => {
    return renderDiffs.map((d, i) => getDiffId({ diff: d, index: i }));
  }, [renderDiffs]);

  const toggle = useCallback((id: string) => {
    setCollapsedIds((prev) => {
      const next = new Set(prev);
      next.has(id) ? next.delete(id) : next.add(id);
      return next;
    });
  }, []);

  const allCollapsed = collapsedIds.size === renderDiffs.length;
  const handleCollapseAll = useCallback(() => {
    setCollapsedIds(allCollapsed ? new Set() : new Set(ids));
  }, [allCollapsed, ids]);

  if (error) {
    return (
      <div className="bg-red-50 border border-red-200 rounded-lg p-4 m-4">
        <div className="text-red-800 text-sm">
          {t('diff.errorLoadingDiff', { error })}
        </div>
      </div>
    );
  }

  return (
    <DiffsPanelContent
      diffs={renderDiffs}
      fileCount={renderSummary.fileCount}
      added={renderSummary.added}
      deleted={renderSummary.deleted}
      collapsedIds={collapsedIds}
      allCollapsed={allCollapsed}
      handleCollapseAll={handleCollapseAll}
      toggle={toggle}
      selectedAttempt={selectedAttempt}
      gitOps={gitOps}
      loading={loading}
      t={t}
    />
  );
}

interface DiffsPanelContentProps {
  diffs: Diff[];
  fileCount: number;
  added: number;
  deleted: number;
  collapsedIds: Set<string>;
  allCollapsed: boolean;
  handleCollapseAll: () => void;
  toggle: (id: string) => void;
  selectedAttempt: Workspace | null;
  gitOps?: GitOperationsInputs;
  loading: boolean;
  t: (key: string, params?: Record<string, unknown>) => string;
}

function DiffsPanelContent({
  diffs,
  fileCount,
  added,
  deleted,
  collapsedIds,
  allCollapsed,
  handleCollapseAll,
  toggle,
  selectedAttempt,
  gitOps,
  loading,
  t,
}: DiffsPanelContentProps) {
  return (
    <div className="h-full flex flex-col relative">
      {diffs.length > 0 && (
        <NewCardHeader
          className="sticky top-0 z-10"
          actions={
            <>
              <DiffViewSwitch />
              <div className="h-4 w-px bg-border" />
              <TooltipProvider>
                <Tooltip>
                  <TooltipTrigger asChild>
                    <Button
                      variant="icon"
                      onClick={handleCollapseAll}
                      aria-pressed={allCollapsed}
                      aria-label={
                        allCollapsed
                          ? t('diff.expandAll')
                          : t('diff.collapseAll')
                      }
                    >
                      {allCollapsed ? (
                        <ChevronsDown className="h-4 w-4" />
                      ) : (
                        <ChevronsUp className="h-4 w-4" />
                      )}
                    </Button>
                  </TooltipTrigger>
                  <TooltipContent side="bottom">
                    {allCollapsed ? t('diff.expandAll') : t('diff.collapseAll')}
                  </TooltipContent>
                </Tooltip>
              </TooltipProvider>
            </>
          }
        >
          <div className="flex items-center">
            <span
              className="text-sm text-muted-foreground whitespace-nowrap"
              aria-live="polite"
            >
              {t('diff.filesChanged', { count: fileCount })}{' '}
              <span className="text-green-600 dark:text-green-500">
                +{added}
              </span>{' '}
              <span className="text-red-600 dark:text-red-500">-{deleted}</span>
            </span>
          </div>
        </NewCardHeader>
      )}
      {gitOps && selectedAttempt && (
        <div className="px-3">
          <GitOperations selectedAttempt={selectedAttempt} {...gitOps} />
        </div>
      )}
      <div className="flex-1 overflow-y-auto px-3">
        {loading ? (
          <div className="flex items-center justify-center h-full">
            <Loader />
          </div>
        ) : diffs.length === 0 ? (
          <div className="flex items-center justify-center h-full text-sm text-muted-foreground">
            {t('diff.noChanges')}
          </div>
        ) : (
          diffs.map((diff, idx) => {
            const id = diff.newPath || diff.oldPath || String(idx);
            return (
              <DiffCard
                key={id}
                diff={diff}
                expanded={!collapsedIds.has(id)}
                onToggle={() => toggle(id)}
                selectedAttempt={selectedAttempt}
              />
            );
          })
        )}
      </div>
    </div>
  );
}
