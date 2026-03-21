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
import React, { useState, useEffect, useCallback } from 'react';
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
  | 'found';           // "Tìm thấy x giao dịch phù hợp"

interface DiscoveryViewProps {
  formData: DynamicFormFields | null;
  onMatchFound?: (count: number, candidates: Candidate[]) => void;
  onStartNegotiation?: (candidates: Candidate[]) => void;
  onCancel?: () => void;
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
}) => {
  const [phase, setPhase] = useState<DiscoveryPhase>('initializing');
  const [progress, setProgress] = useState(0);
  const [matchCount, setMatchCount] = useState(0);
  const [broadcastCount, setBroadcastCount] = useState(0);
  const [statusText, setStatusText] = useState(STATUS_MESSAGES.initializing);
  const [isAnimating, setIsAnimating] = useState(true);
  const [candidates, setCandidates] = useState<Candidate[]>([]);
  const [showCandidateList, setShowCandidateList] = useState(false);

  // Debug log for candidates
  useEffect(() => {
    if (candidates.length > 0) {
      console.log('[DiscoveryView] 📨 Total candidates:', candidates.length);
    }
  }, [candidates]);

  // Discovery flow lock
  const isDiscoveryRunning = React.useRef(false);

  useEffect(() => {
    if (!formData || isDiscoveryRunning.current) return;
    isDiscoveryRunning.current = true;

    const runDiscovery = async () => {
      try {
        // Phase 1: Initializing
        setPhase('initializing');
        setStatusText(STATUS_MESSAGES.initializing);
        setProgress(10);
        await new Promise(resolve => setTimeout(resolve, 1000));

        // Phase 2: Post offer lên server matching engine
        setPhase('joining');
        setStatusText(STATUS_MESSAGES.joining);
        setProgress(30);

        const result = await apiPostOffer(formData.topic, formData.action, formData);
        console.log('[DiscoveryView] Post offer result:', result);

        setPhase('joined');
        setStatusText(STATUS_MESSAGES.joined);
        setProgress(50);
        await new Promise(resolve => setTimeout(resolve, 800));

        // Phase 3: Poll server để lấy offers phù hợp
        setPhase('searching');
        setStatusText(STATUS_MESSAGES.searching);

        const collectedCandidates: Candidate[] = [];
        const processedIds = new Set<string>();

        const mySessionId = getMySessionId();
        const oppositeAction = formData.action === 'buy' ? 'sell' : 'buy';

        // Nếu đã match ngay khi post
        // offer_a = offer đã có sẵn trong store (của đối tác)
        // offer_b = offer vừa post (của user)
        if (result.status === 'matched' && result.match) {
          const match = result.match as any;
          const oppositeOffer = match.offer_a; // luôn là offer của đối tác
          if (oppositeOffer && !processedIds.has(oppositeOffer.session_id)) {
            processedIds.add(oppositeOffer.session_id);
            const candidate = payloadToCandidate({ sender_node_id: oppositeOffer.session_id, form_data: oppositeOffer.form_data }, 0, formData.action);
            collectedCandidates.push(candidate);
            setCandidates([...collectedCandidates]);
            setShowCandidateList(true);
          }
        }

        // Poll 30s để tìm thêm matches
        const maxWait = 30000;
        const pollInterval = 3000;
        const startTime = Date.now();
        let pollCount = 0;

        while (Date.now() - startTime < maxWait) {
          const elapsed = Date.now() - startTime;
          setProgress(50 + Math.round((elapsed / maxWait) * 45));
          setBroadcastCount(++pollCount);

          await new Promise(resolve => setTimeout(resolve, pollInterval));

          try {
            const offers = await apiGetOffers(formData.topic) as any[];
            console.log('[DiscoveryView] Poll', pollCount, '— offers:', offers.length, offers);
            for (const offer of offers) {
              // Chỉ lấy offers của đối tác (action ngược, không phải của mình)
              if (
                offer.action === oppositeAction &&
                offer.session_id !== mySessionId &&
                !processedIds.has(offer.session_id)
              ) {
                processedIds.add(offer.session_id);
                const candidate = payloadToCandidate({ sender_node_id: offer.session_id, form_data: offer.form_data }, collectedCandidates.length, formData.action);
                collectedCandidates.push(candidate);
                setCandidates([...collectedCandidates]);
                setShowCandidateList(true);
              }
            }
          } catch (e) {
            console.error('[DiscoveryView] Poll failed:', e);
          }
        }

        // Phase 4: Done
        setPhase('found');
        setMatchCount(collectedCandidates.length);
        setStatusText(STATUS_MESSAGES.found.replace('{count}', String(collectedCandidates.length)));
        setProgress(100);
        setIsAnimating(false);
        setShowCandidateList(true);
        onMatchFound?.(collectedCandidates.length, collectedCandidates);

      } catch (err) {
        console.error('[DiscoveryView] Error:', err);
      }
    };

    runDiscovery();
  }, [formData, onMatchFound]);

  // Handle start negotiation
  const handleStartNegotiation = (selectedCandidates: Candidate[]) => {
    console.log('[DiscoveryView] Starting negotiation with:', selectedCandidates);
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

  // Show candidate list when we have matches
  if (showCandidateList && candidates.length > 0) {
    return (
      <div className="discovery-view discovery-with-results">
        {/* Compact Status Header - Minimalist Style */}
        <div className="discovery-compact-header">
          <div className="compact-left">
            <div className={`compact-indicator ${phase === 'found' ? 'success' : 'pulsing'}`}>
              <div className="indicator-dot" />
              {phase !== 'found' && <div className="indicator-ping" />}
            </div>
            <div className="compact-status-text">
              <span className="status-primary">
                {phase === 'found' ? 'Discovery Complete' : 'Scanning Network'}
              </span>
              <span className="status-separator">•</span>
              <span className="status-secondary">
                {phase === 'found'
                  ? `${candidates.length} candidates found`
                  : 'Searching for peers...'}
              </span>
            </div>
          </div>

          <div className="compact-right">
            <div className="compact-metric">
              <span className="metric-label">
                {formData.action === 'buy' ? 'Listening' : 'Broadcasts'}:
              </span>
              <span className="metric-value">{broadcastCount}</span>
            </div>
          </div>

          {/* Slim Progress Line at Bottom - CSS animated */}
          {phase !== 'found' && (
            <div className="compact-progress-line">
              <div className="compact-progress-fill" />
            </div>
          )}
        </div>

        {/* Candidate List */}
        <CandidateList
          candidates={candidates}
          formData={formData}
          onStartNegotiation={handleStartNegotiation}
          maxDisplay={10}
        />
      </div>
    );
  }

  return (
    <div className="discovery-view">
      {/* Status Text */}
      <div className={`discovery-status ${phase}`}>
        <span className="status-text">{statusText}</span>
      </div>

      {/* Whale Icon with Ripple Effect */}
      <div className="discovery-visual">
        {/* Ripple circles */}
        <div className={`ripple-container ${isAnimating ? 'animating' : ''}`}>
          <div className="ripple-circle ripple-1" />
          <div className="ripple-circle ripple-2" />
          <div className="ripple-circle ripple-3" />
        </div>

        {/* Center whale icon */}
        <div className="discovery-icon">
          <img src={aiAgentIcon} alt="AI Agent" />
        </div>
      </div>

      {/* Progress Bar */}
      <div className="discovery-progress-container">
        <div className="discovery-progress-bar">
          <div
            className="discovery-progress-fill"
            style={{ width: `${progress}%` }}
          />
        </div>
        <div className="discovery-progress-text">
          {progress}%
        </div>
      </div>

      {/* Broadcast Count */}
      <div className="discovery-broadcast-info">
        <span className="broadcast-label">
          {formData.action === 'buy' ? 'Đang lắng nghe' : 'Đã broadcast'}:
        </span>
        <span className="broadcast-count">{broadcastCount}</span>
        <span className="broadcast-unit">lần</span>
      </div>

      {/* Cancel Button */}
      {phase !== 'found' && (
        <button className="discovery-cancel-btn" onClick={onCancel}>
          Hủy tìm kiếm
        </button>
      )}

      {/* Success Actions */}
      {phase === 'found' && (
        <div className="discovery-success-actions">
          <button
            className="discovery-view-matches-btn"
            onClick={() => setShowCandidateList(true)}
          >
            Xem danh sách phù hợp ({matchCount})
          </button>
        </div>
      )}
    </div>
  );
};

export default DiscoveryView;
