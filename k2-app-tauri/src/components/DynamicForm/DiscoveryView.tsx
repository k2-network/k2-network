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
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
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

  // Get broadcast delay from k2-core (random 1-4s)
  const getBroadcastDelay = useCallback(async (): Promise<number> => {
    try {
      const delay = await invoke<number>('get_broadcast_delay');
      return delay;
    } catch {
      // Fallback: random 1-4 seconds
      return Math.floor(Math.random() * 3000) + 1000;
    }
  }, []);

  // Discovery flow with minimum 2s per phase
  useEffect(() => {
    if (!formData) return;

    const runDiscovery = async () => {
      // Phase 1: Initializing (min 2s)
      setPhase('initializing');
      setStatusText(STATUS_MESSAGES.initializing);
      setProgress(10);
      await new Promise(resolve => setTimeout(resolve, 2000));

      // Phase 2: Joining topic (min 2s)
      setPhase('joining');
      setStatusText(STATUS_MESSAGES.joining);
      setProgress(30);

      // Actually try to join topic via Tauri
      try {
        await invoke('join_topic', {
          topic: formData.topic,
          action: formData.action
        });
      } catch (err) {
        console.log('[DiscoveryView] Join topic (mock):', err);
      }
      await new Promise(resolve => setTimeout(resolve, 2000));

      // Phase 3: Joined (min 2s)
      setPhase('joined');
      setStatusText(STATUS_MESSAGES.joined);
      setProgress(50);
      await new Promise(resolve => setTimeout(resolve, 2000));

      // Phase 4: Searching with broadcasts
      setPhase('searching');
      setStatusText(STATUS_MESSAGES.searching);

      // Start broadcast/listen cycle
      const discoveryLoop = async () => {
        let broadcasts = 0;
        const topic = formData.topic;
        const shouldBroadcast = formData.action === 'sell' || formData.action === 'exchange';
        const collectedCandidates: Candidate[] = [];
        const processedSenderIds = new Set<string>();

        console.log(`[DiscoveryView] 🎧 Starting background listener for ${formData.action}...`);

        const unlisten = await listen<any>('k2://offer-received', async (event) => {
          const payload = event.payload;
          console.log('[DiscoveryView] 📨 Message received:', payload);

          if (!payload || !payload.sender_node_id) return;

          // If I am a BUYER, I look for "offer"
          if (!shouldBroadcast && payload.message_type === 'offer') {
            if (!processedSenderIds.has(payload.sender_node_id)) {
              processedSenderIds.add(payload.sender_node_id);
              const candidate = payloadToCandidate(payload, collectedCandidates.length, formData.action);
              collectedCandidates.push(candidate);
              setCandidates([...collectedCandidates]);
              setBroadcastCount(collectedCandidates.length);

              // ⚡ AUTO-REPLY TO SELLER
              console.log(`[DiscoveryView] 🤝 Found offer! Sending interest to: ${payload.sender_node_id}`);
              try {
                await invoke('send_interest', { topic, sellerNodeId: payload.sender_node_id, formData });
              } catch (e) { console.error('Reply failed:', e); }
            }
          }

          // If I am a SELLER, I look for "interest" targeting ME
          if (shouldBroadcast && payload.message_type === 'interest') {
            console.log(`[DiscoveryView] 💰 A buyer is interested!`);
            if (!processedSenderIds.has(payload.sender_node_id)) {
              processedSenderIds.add(payload.sender_node_id);
              const candidate = payloadToCandidate(payload, collectedCandidates.length, formData.action);
              collectedCandidates.push(candidate);
              setCandidates([...collectedCandidates]);
              setBroadcastCount(collectedCandidates.length);
            }
          }
        });

        try {
          // Start background task in Rust
          await invoke('start_listening', { topic });

          const startTime = Date.now();
          const maxWait = 30000; // Wait 30s (reduced from 45s)
          const minCandidates = 1; // Show list when at least 1 found

          while (Date.now() - startTime < maxWait) {
            const now = Date.now();
            const elapsed = now - startTime;
            setProgress(50 + Math.min((elapsed / maxWait) * 45, 45));

            // Show candidate list as soon as we have at least 1
            if (collectedCandidates.length >= minCandidates && !showCandidateList) {
              setShowCandidateList(true);
            }

            if (shouldBroadcast) {
              // Seller: Periodic broadcast
              if (elapsed % 5000 < 500) { // Every ~5s
                broadcasts++;
                setBroadcastCount(broadcasts);
                try {
                  await invoke('broadcast_offer', { topic, formData });
                  console.log(`[DiscoveryView] 📡 Broadcast #${broadcasts} sent`);
                } catch (e) {
                  console.log('[DiscoveryView] Broadcast (mock):', e);
                }
              }
            }

            await new Promise(resolve => setTimeout(resolve, 500));
          }

        } finally {
          unlisten();
        }

        // NO MOCK - Show real P2P results only
        // If no candidates found, user will see "0 matches found"
        console.log(`[DiscoveryView] Discovery complete. Found ${collectedCandidates.length} real P2P candidates.`);

        return collectedCandidates;
      };

      const foundCandidates = await discoveryLoop();

      // Phase 5: Found matches
      setPhase('found');
      setMatchCount(foundCandidates.length);
      setStatusText(STATUS_MESSAGES.found.replace('{count}', String(foundCandidates.length)));
      setProgress(100);
      setIsAnimating(false);
      setShowCandidateList(true);

      if (onMatchFound) {
        onMatchFound(foundCandidates.length, foundCandidates);
      }
    };

    runDiscovery();
  }, [formData, getBroadcastDelay, onMatchFound]);

  // Handle start negotiation
  const handleStartNegotiation = (selectedCandidates: Candidate[]) => {
    console.log('[DiscoveryView] Starting negotiation with:', selectedCandidates);
    onStartNegotiation?.(selectedCandidates);
  };

  if (!formData) {
    return (
      <div className="discovery-view-empty">
        <p>Chưa có yêu cầu nào được tạo.</p>
        <p>Vui lòng tạo yêu cầu mua/bán/trao đổi trước.</p>
      </div>
    );
  }

  // Show candidate list when we have matches
  if (showCandidateList && candidates.length > 0) {
    return (
      <div className="discovery-view discovery-with-results">
        {/* Compact Status Header */}
        <div className="discovery-compact-header">
          <div className="compact-status">
            <div className="compact-icon">
              <img src={aiAgentIcon} alt="AI Agent" />
            </div>
            <div className="compact-info">
              <span className="compact-title">
                {phase === 'found'
                  ? `Found ${candidates.length} matches`
                  : 'Still searching...'}
              </span>
              <span className="compact-subtitle">
                {formData.action === 'buy' ? 'Listening' : 'Broadcasting'}: {broadcastCount}
              </span>
            </div>
          </div>

          {phase !== 'found' && (
            <div className="compact-progress">
              <div
                className="compact-progress-fill"
                style={{ width: `${progress}%` }}
              />
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
