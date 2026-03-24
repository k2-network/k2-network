import React, { useState, useEffect, useCallback, useRef } from 'react';
import { getSubtopicStats, announceTopic } from '../../api/tracker';
import type { SubtopicStat } from '../../api/tracker';
import { k2ws } from '../../api/ws';
import { getMyNodeId } from '../../api/node';
import './SubtopicDashboard.css';

const ACTION_COLORS = {
  buy:      '#47E069',
  sell:     '#FF6B6B',
  exchange: '#4DA6FF',
  unknown:  '#6B7280',
} as const;

type ActionKey = keyof typeof ACTION_COLORS;

interface SubtopicDashboardProps {
  topic: string;
  subtopic: string;
  accentColor: string;
  onClose: () => void;
  /** action từ AI classify: "buy" | "sell" | "exchange" — nếu có sẽ announce lên tracker */
  action?: string;
}

export const SubtopicDashboard: React.FC<SubtopicDashboardProps> = ({
  topic,
  subtopic,
  accentColor,
  onClose,
  action,
}) => {
  const [stats, setStats] = useState<SubtopicStat[]>([]);
  const [loading, setLoading] = useState(true);
  const [lastRefresh, setLastRefresh] = useState<number>(0);
  const intervalRef = useRef<ReturnType<typeof setInterval> | null>(null);

  // Announce presence when entering subtopic view (with action if known)
  useEffect(() => {
    getMyNodeId()
      .then((nodeId: string) => {
        if (nodeId) announceTopic(topic, nodeId, subtopic, action);
      })
      .catch(() => {});
  }, [topic, subtopic, action]);

  const fetchStats = useCallback(async () => {
    try {
      const res = await getSubtopicStats(topic);
      setStats(res.stats);
    } catch {
      // keep stale data on error
    } finally {
      setLoading(false);
      setLastRefresh(Date.now());
    }
  }, [topic]);

  useEffect(() => {
    fetchStats();
    intervalRef.current = setInterval(fetchStats, 10_000);
    return () => { if (intervalRef.current) clearInterval(intervalRef.current); };
  }, [fetchStats]);

  // WS live push
  useEffect(() => {
    const unlisten = k2ws.listen('k2://subtopic-stats-updated', (payload: unknown) => {
      const p = payload as { topic?: string; stats?: SubtopicStat[] };
      if (p?.topic === topic && p.stats) {
        setStats(p.stats);
        setLastRefresh(Date.now());
      }
    });
    return unlisten;
  }, [topic]);

  const focused = stats.find((s) => s.subtopic === subtopic);
  const maxTotal = Math.max(...stats.map((s) => s.total), 1);

  // Build conic-gradient for donut
  const buildDonut = (stat?: SubtopicStat) => {
    if (!stat || stat.total === 0) return '#2a2a3a 0deg 360deg';
    const t = stat.total;
    const bd = (stat.buy      / t) * 360;
    const sd = (stat.sell     / t) * 360;
    const ed = (stat.exchange / t) * 360;
    return [
      `${ACTION_COLORS.buy}      0deg   ${bd}deg`,
      `${ACTION_COLORS.sell}     ${bd}deg   ${bd + sd}deg`,
      `${ACTION_COLORS.exchange} ${bd + sd}deg ${bd + sd + ed}deg`,
      `${ACTION_COLORS.unknown}  ${bd + sd + ed}deg 360deg`,
    ].join(', ');
  };

  const allNodes: Array<{ id: string; action: ActionKey }> = focused
    ? [
        ...focused.nodes.buy.map(id => ({ id, action: 'buy' as ActionKey })),
        ...focused.nodes.sell.map(id => ({ id, action: 'sell' as ActionKey })),
        ...focused.nodes.exchange.map(id => ({ id, action: 'exchange' as ActionKey })),
        ...focused.nodes.unknown.map(id => ({ id, action: 'unknown' as ActionKey })),
      ]
    : [];

  const secsAgo = lastRefresh ? Math.round((Date.now() - lastRefresh) / 1000) : null;

  return (
    <div className="sd-root">
      {/* ── Header ── */}
      <div className="sd-header">
        <div className="sd-header-left">
          <button className="sd-back-btn" onClick={onClose}>
            <svg width="14" height="14" viewBox="0 0 14 14" fill="none">
              <path d="M9 11L5 7L9 3" stroke="currentColor" strokeWidth="1.5"
                strokeLinecap="round" strokeLinejoin="round"/>
            </svg>
            Back
          </button>
          <h3 className="sd-title" style={{ color: accentColor }}>{subtopic}</h3>
          <span className="sd-badge">{topic}</span>
        </div>
        <div className="sd-live-pill">
          <span className="sd-live-dot" />
          {secsAgo !== null ? `${secsAgo}s ago` : 'loading...'}
        </div>
      </div>

      {loading ? (
        <div className="sd-skeletons">
          <div className="sd-skel sd-skel-stats" />
          <div className="sd-skel sd-skel-chart" />
        </div>
      ) : (
        <>
          {/* ── Stat Cards ── */}
          <div className="sd-stat-row">
            {(['buy', 'sell', 'exchange'] as ActionKey[]).map((act) => (
              <div key={act} className="sd-stat-card">
                <div className="sd-stat-ring" style={{ '--rc': ACTION_COLORS[act] } as React.CSSProperties}>
                  <span className="sd-stat-val">{focused?.[act] ?? 0}</span>
                </div>
                <span className="sd-stat-lbl">{act.charAt(0).toUpperCase() + act.slice(1)}</span>
              </div>
            ))}
            <div className="sd-stat-card sd-stat-card--total">
              <div className="sd-stat-ring" style={{ '--rc': accentColor } as React.CSSProperties}>
                <span className="sd-stat-val">{focused?.total ?? 0}</span>
              </div>
              <span className="sd-stat-lbl">Total</span>
            </div>
          </div>

          {/* ── Bar Chart ── */}
          <div className="sd-section">
            <p className="sd-section-lbl">Activity across sub-categories</p>
            <div className="sd-bars">
              {stats.length === 0 && (
                <p className="sd-empty">No activity yet — be the first to announce!</p>
              )}
              {stats.slice(0, 14).map((s) => {
                const isFocused = s.subtopic === subtopic;
                return (
                  <div key={s.subtopic} className={`sd-bar-row${isFocused ? ' sd-bar-row--active' : ''}`}>
                    <span className="sd-bar-label" title={s.subtopic}>
                      {isFocused && <span className="sd-bar-dot" style={{ background: accentColor }} />}
                      {s.subtopic}
                    </span>
                    <div className="sd-bar-track">
                      {(['buy', 'sell', 'exchange'] as ActionKey[]).map((act) => {
                        const pct = (s[act] / maxTotal) * 100;
                        return pct > 0 ? (
                          <div key={act} className="sd-bar-seg"
                            style={{ width: `${pct}%`, background: ACTION_COLORS[act] }}
                            title={`${act}: ${s[act]}`} />
                        ) : null;
                      })}
                    </div>
                    <span className="sd-bar-count">{s.total}</span>
                  </div>
                );
              })}
            </div>
          </div>

          {/* ── Donut + Pulse ── */}
          <div className="sd-section sd-row-section">
            {/* Donut */}
            <div className="sd-donut-block">
              <p className="sd-section-lbl">Buy / Sell / Exchange ratio</p>
              <div className="sd-donut" style={{
                background: `conic-gradient(${buildDonut(focused)})`,
              }}>
                <div className="sd-donut-hole" style={{ background: '#131320' }}>
                  <span className="sd-donut-val">{focused?.total ?? 0}</span>
                  <span className="sd-donut-sub">nodes</span>
                </div>
              </div>
              <div className="sd-legend">
                {(['buy', 'sell', 'exchange', 'unknown'] as ActionKey[]).map((act) => (
                  <div key={act} className="sd-legend-item">
                    <span className="sd-legend-dot" style={{ background: ACTION_COLORS[act] }} />
                    <span className="sd-legend-text">
                      {act.charAt(0).toUpperCase() + act.slice(1)}
                      <span className="sd-legend-count"> {focused?.[act] ?? 0}</span>
                    </span>
                  </div>
                ))}
              </div>
            </div>

            {/* Pulse Ring */}
            <div className="sd-pulse-block">
              <p className="sd-section-lbl">Live Nodes</p>
              <div className="sd-pulse-wrap">
                <div className="sd-pulse-ring sd-pr-outer"
                  style={{ '--pc': accentColor } as React.CSSProperties} />
                <div className="sd-pulse-ring sd-pr-mid"
                  style={{ '--pc': accentColor } as React.CSSProperties} />
                <div className="sd-pulse-core"
                  style={{ background: `radial-gradient(circle, ${accentColor}cc, ${accentColor}66)` }}>
                  <span className="sd-pulse-num">{focused?.total ?? 0}</span>
                  <span className="sd-pulse-sub">active</span>
                </div>
              </div>
            </div>
          </div>

          {/* ── Node List ── */}
          <div className="sd-section">
            <p className="sd-section-lbl">Nodes in "{subtopic}"</p>
            {allNodes.length === 0 ? (
              <p className="sd-empty">No nodes announced yet in this sub-category.</p>
            ) : (
              <div className="sd-node-list">
                {allNodes.slice(0, 20).map(({ id, action }, i) => (
                  <div key={`${id}-${i}`} className="sd-node-row">
                    <span className="sd-node-dot" style={{ background: ACTION_COLORS[action] }} />
                    <span className="sd-node-id">{id.slice(0, 8)}…{id.slice(-6)}</span>
                    <span className="sd-node-action" style={{ color: ACTION_COLORS[action] }}>
                      {action}
                    </span>
                  </div>
                ))}
                {allNodes.length > 20 && (
                  <p className="sd-more">+{allNodes.length - 20} more nodes</p>
                )}
              </div>
            )}
          </div>
        </>
      )}
    </div>
  );
};
