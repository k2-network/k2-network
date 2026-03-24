/**
 * DiscoveryView - Finding Match UI
 * 
 * Displays the P2P discovery process with:
 * - AI whale icon with ripple animation
 * - Status messages with smooth transitions
 * - Progress bar for matching
 * - Broadcast count display
 * - Real-time offer reception via Tauri events
 * - CandidateList when matches are found
 */
import React, { useState, useEffect, useCallback, useRef } from 'react';
import { postOffer as apiPostOffer, getOffers as apiGetOffers, getMySessionId } from '../../api';
import aiAgentIcon from '../../assets/icons/ai-agent-large-dark.svg';
import { CandidateList } from './CandidateList';
import './DiscoveryView.css';
import type { DynamicFormFields, Candidate } from './types';

// Status phases for discovery
type DiscoveryPhase =
  | 'initializing'     // "Let me find the right match for you"
  | 'joining'          // "Joining topic..."
  | 'joined'           // "Successfully join topic"
  | 'searching'        // "Searching for matches..."
  | 'found';           // "Đang tìm kiếm — x matches"

interface DiscoveryViewProps {
  formData: DynamicFormFields | null;
  onMatchFound?: (count: number, candidates: Candidate[]) => void;
  onStartNegotiation?: (candidates: Candidate[]) => void;
  onCancel?: () => void;
  /** Ref nhận hàm stopPolling — gọi khi deal xong để dừng tìm kiếm */
  stopPollingRef?: React.MutableRefObject<(() => void) | null>;
}

// Status messages
const STATUS_MESSAGES: Record<DiscoveryPhase, string> = {
  initializing: 'Let me find the right match for you',
  joining: 'Joining topic...',
  joined: 'Successfully join topic',
  searching: 'Searching for matches...',
  found: 'Tìm thấy {count} giao dịch phù hợp',
};

// Convert P2P payload to Candidate
// Payload format from Rust broadcast_offer:
// { sender_node_id, message_type, topic, form_data: { title, priceRange, location, ... }, timestamp }
const payloadToCandidate = (payload: any, index: number, formAction: string): Candidate => {
  // Determine opposite action (if user is buying, candidates are selling)
  const candidateAction = formAction === 'buy' ? 'sell' :
    formAction === 'sell' ? 'buy' : 'exchange';

  // Extract form_data from payload (real P2P data)
  const formData = payload.form_data || payload;

  // Parse price from form_data.priceRange
  const priceRange = formData.priceRange || {};
  const priceMin = priceRange.min ?? formData.price_min ?? 0;
  const priceMax = priceRange.max ?? formData.price_max ?? priceMin;
  const currency = priceRange.currency ?? 'USD';

  // Generate short name from nodeId if no name provided
  const nodeId = payload.sender_node_id || `unknown-${index}`;
  const shortNodeId = nodeId.length > 8 ? nodeId.substring(0, 8) : nodeId;
  const name = formData.sender_name || `Peer ${shortNodeId}`;

  return {
    nodeId: nodeId,
    name: name,
    title: formData.title || formData.description || `Offer from ${shortNodeId}`,
    action: candidateAction as any,
    status: 'active', // Real P2P means they're online
    matchScore: formData.match_score || 0.8 + Math.random() * 0.2, // High score for real matches
    priceRange: {
      min: priceMin,
      max: priceMax,
      currency: currency
    },
    location: formData.location || 'P2P Network',
    topic: payload.topic || formData.topic || 'Goods',
    description: formData.description || '',
  };
};

export const DiscoveryView: React.FC<DiscoveryViewProps> = ({
  formData,
  onMatchFound,
  onStartNegotiation,
  onCancel,
  stopPollingRef,
}) => {
  const [phase, setPhase] = useState<DiscoveryPhase>('initializing');
  const [progress, setProgress] = useState(0);
  const [broadcastCount, setBroadcastCount] = useState(0);
  const [statusText, setStatusText] = useState(STATUS_MESSAGES.initializing);
  const [isAnimating, setIsAnimating] = useState(true);
  const [candidates, setCandidates] = useState<Candidate[]>([]);
  const [showCandidateList, setShowCandidateList] = useState(false);
  const [postDone, setPostDone] = useState(false);

  // Abort controller để dừng poll từ bên ngoài
  const abortRef = React.useRef<AbortController | null>(null);
  const isDiscoveryRunning = React.useRef(false);

  // Stable ref for onMatchFound — prevents useEffect from restarting on every render
  const onMatchFoundRef = useRef(onMatchFound);
  useEffect(() => { onMatchFoundRef.current = onMatchFound; }, [onMatchFound]);

  // Expose stopPolling lên parent qua ref
  useEffect(() => {
    if (stopPollingRef) {
      stopPollingRef.current = () => {
        abortRef.current?.abort();
      };
    }
    return () => {
      if (stopPollingRef) stopPollingRef.current = null;
    };
  }, [stopPollingRef]);

  // Cleanup khi unmount
  useEffect(() => {
    return () => { abortRef.current?.abort(); };
  }, []);

  useEffect(() => {
    if (!formData) return;
    // Abort bất kỳ discovery nào đang chạy trước đó
    abortRef.current?.abort();
    isDiscoveryRunning.current = false;

    // Reset state cho lần chạy mới
    setPhase('initializing');
    setProgress(0);
    setBroadcastCount(0);
    setStatusText(STATUS_MESSAGES.initializing);
    setIsAnimating(true);
    setCandidates([]);
    setShowCandidateList(false);
    setPostDone(false);

    isDiscoveryRunning.current = true;
    const abort = new AbortController();
    abortRef.current = abort;

    const runDiscovery = async () => {
      try {
        // Phase 1: Initializing
        setPhase('initializing');
        setStatusText(STATUS_MESSAGES.initializing);
        setProgress(10);
        await new Promise(resolve => setTimeout(resolve, 800));
        if (abort.signal.aborted) return;

        // Phase 2: Post offer
        setPhase('joining');
        setStatusText(STATUS_MESSAGES.joining);
        setProgress(30);

        const result = await apiPostOffer(formData.topic, formData.action, formData);
        console.log('[DiscoveryView] Post offer result:', result);
        if (abort.signal.aborted) return;

        setPhase('joined');
        setStatusText(STATUS_MESSAGES.joined);
        setProgress(50);
        setPostDone(true);
        await new Promise(resolve => setTimeout(resolve, 600));
        if (abort.signal.aborted) return;

        // Phase 3: Poll vô hạn cho đến khi bị abort
        setPhase('searching');
        setStatusText(STATUS_MESSAGES.searching);

        // Map sessionId → candidate để merge không trùng
        const candidateMap = new Map<string, Candidate>();
        const mySessionId = getMySessionId();
        const oppositeAction = formData.action === 'buy' ? 'sell'
          : formData.action === 'sell' ? 'buy' : 'exchange';
        let pollCount = 0;
        let lastPostTime = Date.now();

        // Hàm lấy snapshot candidates từ map, sort theo matchScore
        const snapshot = () => [...candidateMap.values()].sort((a, b) => (b.matchScore ?? 0) - (a.matchScore ?? 0));

        // Nếu match ngay khi post
        if (result.status === 'matched' && result.match) {
          const match = result.match as any;
          const oppositeOffer = match.offer_a;
          if (oppositeOffer && !candidateMap.has(oppositeOffer.session_id)) {
            const candidate = payloadToCandidate({ sender_node_id: oppositeOffer.session_id, form_data: oppositeOffer.form_data }, 0, formData.action);
            candidateMap.set(oppositeOffer.session_id, candidate);
            const list = snapshot();
            setCandidates(list);
            setShowCandidateList(true);
            onMatchFound?.(list.length, list);
          }
        }

        // Poll liên tục 5s/lần — không có timeout cứng
        while (!abort.signal.aborted) {
          await new Promise(resolve => setTimeout(resolve, 5000));
          if (abort.signal.aborted) break;

          setBroadcastCount(++pollCount);
          setProgress(55 + Math.round(Math.sin(pollCount * 0.4) * 20 + 20));

          // Auto-refresh offer mỗi 4 phút để tránh TTL 5 phút trên server
          if (Date.now() - lastPostTime > 4 * 60 * 1000) {
            try {
              await apiPostOffer(formData.topic, formData.action, formData);
              lastPostTime = Date.now();
            } catch (e) {
              if (!abort.signal.aborted) console.error('[DiscoveryView] Re-post failed:', e);
            }
            if (abort.signal.aborted) break;
          }

          try {
            const offers = await apiGetOffers(formData.topic) as any[];
            let hasNew = false;
            for (const offer of offers) {
              // Chỉ lọc theo action + không phải mình — KHÔNG lọc subtopic vì AI classify có thể khác nhau
              if (
                offer.action === oppositeAction &&
                offer.session_id !== mySessionId &&
                !candidateMap.has(offer.session_id)
              ) {
                const candidate = payloadToCandidate(
                  { sender_node_id: offer.session_id, form_data: offer.form_data },
                  candidateMap.size,
                  formData.action
                );
                candidateMap.set(offer.session_id, candidate);
                hasNew = true;
              }
            }

            if (hasNew) {
              const list = snapshot();
              setCandidates(list);
              setShowCandidateList(true);
              onMatchFoundRef.current?.(list.length, list);
            }
          } catch (e) {
            if (!abort.signal.aborted) console.error('[DiscoveryView] Poll failed:', e);
          }

          // Cập nhật status text với số lượng realtime
          if (!abort.signal.aborted) {
            const count = candidateMap.size;
            const msg = count > 0
              ? STATUS_MESSAGES.found.replace('{count}', String(count))
              : STATUS_MESSAGES.searching;
            setStatusText(msg);
          }
        }

      } catch (err) {
        if (!abort.signal.aborted) console.error('[DiscoveryView] Error:', err);
      }
    };

    runDiscovery();
  }, [formData]); // onMatchFound accessed via ref — prevents restart on every render

  const handleStartNegotiation = (selectedCandidates: Candidate[]) => {
    onStartNegotiation?.(selectedCandidates);
  };

  if (!formData) {
    return (
      <div className="discovery-view-empty">
        <span>Chưa có yêu cầu nào được tạo.</span>
        <span>Vui lòng tạo yêu cầu mua/bán/trao đổi trước.</span>
      </div>
    );
  }

  // Compact status bar — luôn hiển thị phía trên khi đang searching
  const statusBar = postDone && (
    <div className="discovery-compact-header">
      <div className="compact-left">
        <div className="compact-indicator pulsing">
          <div className="indicator-dot" />
          <div className="indicator-ping" />
        </div>
        <div className="compact-status-text">
          <span className="status-primary">Đang quét mạng P2P</span>
          <span className="status-separator">•</span>
          <span className="status-secondary">
            {candidates.length > 0
              ? `${candidates.length} kết quả — tiếp tục tìm thêm`
              : 'Đang tìm kiếm đối tác...'}
          </span>
        </div>
      </div>
      <div className="compact-right">
        <div className="compact-metric">
          <span className="metric-label">Polls:</span>
          <span className="metric-value">{broadcastCount}</span>
        </div>
        <button className="discovery-cancel-btn-inline" onClick={onCancel}>
          Hủy
        </button>
      </div>
      <div className="compact-progress-line">
        <div className="compact-progress-fill" />
      </div>
    </div>
  );

  // Nếu đã có candidates → hiện danh sách + status bar bên trên
  if (showCandidateList && candidates.length > 0) {
    return (
      <div className="discovery-view discovery-with-results">
        {statusBar}
        <CandidateList
          candidates={candidates}
          formData={formData}
          onStartNegotiation={handleStartNegotiation}
          maxDisplay={10}
        />
      </div>
    );
  }

  // Chưa có candidates → hiện animation tìm kiếm
  return (
    <div className="discovery-view">
      <div className={`discovery-status ${phase}`}>
        <span className="status-text">{statusText}</span>
      </div>

      <div className="discovery-visual">
        <div className={`ripple-container ${isAnimating ? 'animating' : ''}`}>
          <div className="ripple-circle ripple-1" />
          <div className="ripple-circle ripple-2" />
          <div className="ripple-circle ripple-3" />
        </div>
        <div className="discovery-icon">
          <img src={aiAgentIcon} alt="AI Agent" />
        </div>
      </div>

      <div className="discovery-progress-container">
        <div className="discovery-progress-bar">
          <div className="discovery-progress-fill" style={{ width: `${progress}%` }} />
        </div>
        <div className="discovery-progress-text">{progress}%</div>
      </div>

      <div className="discovery-broadcast-info">
        <span className="broadcast-label">
          {formData.action === 'buy' ? 'Đang lắng nghe' : 'Đã broadcast'}:
        </span>
        <span className="broadcast-count">{broadcastCount}</span>
        <span className="broadcast-unit">lần</span>
      </div>

      <button className="discovery-cancel-btn" onClick={onCancel}>
        Hủy tìm kiếm
      </button>
    </div>
  );
};

export default DiscoveryView;
