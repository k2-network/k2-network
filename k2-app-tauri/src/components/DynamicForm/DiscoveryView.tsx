/**
 * DiscoveryView - Finding Match UI
 * 
 * Displays the P2P discovery process with:
 * - AI whale icon with ripple animation
 * - Status messages with smooth transitions
 * - Progress bar for matching
 * - Broadcast count display
 * - Real-time offer reception via Tauri events
 */
import React, { useState, useEffect, useCallback } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import aiAgentIcon from '../../assets/icons/ai-agent-large-dark.svg';
import './DiscoveryView.css';
import type { DynamicFormFields } from './types';

// Status phases for discovery
type DiscoveryPhase = 
  | 'initializing'     // "Let me find the right match for you"
  | 'joining'          // "Joining topic..."
  | 'joined'           // "Successfully join topic"
  | 'searching'        // "Searching for matches..."
  | 'found';           // "Tìm thấy x giao dịch phù hợp"

interface DiscoveryViewProps {
  formData: DynamicFormFields | null;
  onMatchFound?: (count: number) => void;
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

export const DiscoveryView: React.FC<DiscoveryViewProps> = ({
  formData,
  onMatchFound,
  onCancel,
}) => {
  const [phase, setPhase] = useState<DiscoveryPhase>('initializing');
  const [progress, setProgress] = useState(0);
  const [matchCount, setMatchCount] = useState(0);
  const [broadcastCount, setBroadcastCount] = useState(0);
  const [statusText, setStatusText] = useState(STATUS_MESSAGES.initializing);
  const [isAnimating, setIsAnimating] = useState(true);
  const [receivedOffers, setReceivedOffers] = useState<any[]>([]);

  // Debug log for received offers
  useEffect(() => {
    if (receivedOffers.length > 0) {
        console.log('[DiscoveryView] 📨 Total offers received:', receivedOffers.length);
    }
  }, [receivedOffers]);

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
        let found = 0;
        const topic = formData.topic;
        const shouldBroadcast = formData.action === 'sell' || formData.action === 'exchange';

        // 🎧 ALWAYS START LISTENING (Both Buyer & Seller)
        // This keeps the P2P connection alive and handles messages
        console.log(`[DiscoveryView] 🎧 Starting background listener for ${formData.action}...`);
        const offersReceived: any[] = [];
        const processedSenderIds = new Set<string>();

        const unlisten = await listen<any>('k2://offer-received', async (event) => {
          const payload = event.payload;
          console.log('[DiscoveryView] 📨 Message received:', payload);
          
          if (!payload || !payload.sender_node_id) return;

          // If I am a BUYER, I look for "offer"
          if (!shouldBroadcast && payload.message_type === 'offer') {
             if (!processedSenderIds.has(payload.sender_node_id)) {
                processedSenderIds.add(payload.sender_node_id);
                offersReceived.push(payload);
                setReceivedOffers(prev => [...prev, payload]);
                setBroadcastCount(offersReceived.length);
                
                // ⚡ AUTO-REPLY TO SELLER
                console.log(`[DiscoveryView] 🤝 Found offer! Sending interest to: ${payload.sender_node_id}`);
                try {
                  await invoke('send_interest', { topic, sellerNodeId: payload.sender_node_id, formData });
                } catch (e) { console.error('Reply failed:', e); }
             }
          }
          
          // If I am a SELLER, I look for "interest" targeting ME
          if (shouldBroadcast && payload.message_type === 'interest') {
             // In a real app, we'd check if target_node_id matches our own
             console.log(`[DiscoveryView] 💰 A buyer is interested!`);
             if (!processedSenderIds.has(payload.sender_node_id)) {
                processedSenderIds.add(payload.sender_node_id);
                offersReceived.push(payload);
                setReceivedOffers(prev => [...prev, payload]);
                setBroadcastCount(offersReceived.length);
             }
          }
        });

        try {
          // Start background task in Rust
          await invoke('start_listening', { topic });

          const startTime = Date.now();
          const maxWait = 45000; // Wait 45s

          while (offersReceived.length === 0 && Date.now() - startTime < maxWait) {
            const now = Date.now();
            const elapsed = now - startTime;
            setProgress(50 + Math.min((elapsed / maxWait) * 45, 45));

            if (shouldBroadcast) {
               // Seller: Periodic broadcast
               if (elapsed % 5000 < 500) { // Every ~5s
                  broadcasts++;
                  setBroadcastCount(broadcasts);
                  await invoke('broadcast_offer', { topic, formData });
                  console.log(`[DiscoveryView] 📡 Broadcast #${broadcasts} sent`);
               }
            }
            
            await new Promise(resolve => setTimeout(resolve, 500));
          }
          
          found = offersReceived.length;
        } finally {
          unlisten();
        }
        
        return found;
      };


      const foundCount = await discoveryLoop();
      
      // Phase 5: Found matches
      setPhase('found');
      setMatchCount(foundCount);
      setStatusText(STATUS_MESSAGES.found.replace('{count}', String(foundCount)));
      setProgress(100);
      setIsAnimating(false);
      
      if (onMatchFound) {
        onMatchFound(foundCount);
      }
    };

    runDiscovery();
  }, [formData, getBroadcastDelay, onMatchFound]);

  if (!formData) {
    return (
      <div className="discovery-view-empty">
        <p>Chưa có yêu cầu nào được tạo.</p>
        <p>Vui lòng tạo yêu cầu mua/bán/trao đổi trước.</p>
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
          <button className="discovery-view-matches-btn">
            Xem danh sách phù hợp ({matchCount})
          </button>
        </div>
      )}
    </div>
  );
};

export default DiscoveryView;
