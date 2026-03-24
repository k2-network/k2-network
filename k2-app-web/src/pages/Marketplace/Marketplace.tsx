import React, { useState, useEffect, useCallback, useRef } from "react";
import { SubtopicDashboard } from "../../components/SubtopicDashboard/SubtopicDashboard";
import { postOffer as apiPostOffer, getOffers as apiGetOffers, cancelOffer as apiCancelOffer, announceTopic, leaveTopic as apiLeaveTopic, getMyNodeId } from "../../api";
import "./Marketplace.css";
import { MarketplaceTabs, DynamicRequestForm, SkeletonForm, DiscoveryView, NegotiationDashboard, FindMatchingView } from "../../components/DynamicForm";
import type { TabType, Candidate, ActiveOffer } from "../../components/DynamicForm";
import type { DynamicFormFields } from "../../components/DynamicForm";
// Digital Digital Assets Icons
import videoIcon from "../../assets/icons/video.svg";
import imagesIcon from "../../assets/icons/images.svg";
import audioIcon from "../../assets/icons/audio.svg";
import tokenIcon from "../../assets/icons/token.svg";
import keyIcon from "../../assets/icons/key.svg";
import documentIcon from "../../assets/icons/document.svg";
import sourceCodeIcon from "../../assets/icons/source-code.svg";
import datasetIcon from "../../assets/icons/dataset.svg";
// Goods Icons
import fashionIcon from "../../assets/icons/fashion.svg";
import electronicsIcon from "../../assets/icons/electronics-devices.svg";
import booksIcon from "../../assets/icons/books-learning.svg";
import sportsIcon from "../../assets/icons/sports-travel.svg";
// Freelance Icons
import techIcon from "../../assets/icons/tech-it.svg";
import designIcon from "../../assets/icons/design-creative.svg";
import translateIcon from "../../assets/icons/translate.svg";
import marketingIcon from "../../assets/icons/marketing-sale.svg";

type CategoryItem = {
    id: string;
    name: string;
    traders: string;
    icon: string;
    accentColor: string;
    subCategories: string[];
    description: string;
};

type FreelanceItem = {
    id: string;
    name: string;
    jobs: string;
    icon: string;
    accentColor: string;
    details: string[];
    description: string;
};

type SelectedCategory = {
    type: 'digital' | 'goods' | 'freelance';
    item: CategoryItem | FreelanceItem;
};

// Category data based on Figma design
const digitalAssets: CategoryItem[] = [
    {
        id: "video", name: "Video", traders: "1k active traders", icon: "video",
        accentColor: "#FF6B6B",
        subCategories: ["Short Clips", "Full Movies", "Tutorials", "Stock Footage", "Animations", "Live Streams"],
        description: "Buy, sell or exchange video content — from stock footage to full productions.",
    },
    {
        id: "images", name: "Images", traders: "1k active traders", icon: "image",
        accentColor: "#4ECDC4",
        subCategories: ["Photography", "Illustrations", "Vector Art", "Icons & UI Kits", "Wallpapers", "NFT Artwork"],
        description: "Trade high-quality images, photos, and visual assets.",
    },
    {
        id: "audio", name: "Audio", traders: "1k active traders", icon: "audio",
        accentColor: "#A78BFA",
        subCategories: ["Music Tracks", "Sound Effects", "Podcasts", "Voice Overs", "Samples & Loops", "ASMR"],
        description: "Discover and trade audio files, music, and sound effects.",
    },
    {
        id: "token", name: "Token", traders: "1k active traders", icon: "token",
        accentColor: "#F59E0B",
        subCategories: ["ERC-20 Tokens", "NFTs", "Game Tokens", "Utility Tokens", "Governance Tokens", "Stablecoins"],
        description: "Exchange digital tokens and blockchain-based assets.",
    },
    {
        id: "license", name: "License | Key | Secret", traders: "1k active traders", icon: "key",
        accentColor: "#10B981",
        subCategories: ["Software Licenses", "API Keys", "Game Keys", "Subscription Access", "Domain Access", "SSL Certificates"],
        description: "Securely trade software licenses, keys, and digital access credentials.",
    },
    {
        id: "document", name: "Document", traders: "1k active traders", icon: "document",
        accentColor: "#3B82F6",
        subCategories: ["Templates", "Research Papers", "E-Books", "Legal Documents", "Business Plans", "Whitepapers"],
        description: "Find and share documents, templates, and written resources.",
    },
    {
        id: "source-code", name: "Source Code", traders: "1k active traders", icon: "code",
        accentColor: "#F15CDD",
        subCategories: ["Full Projects", "Scripts & Snippets", "Libraries", "Plugins", "Themes", "Bots & Automation"],
        description: "Buy and sell source code, scripts, libraries, and software projects.",
    },
    {
        id: "dataset", name: "Dataset", traders: "1k active traders", icon: "data",
        accentColor: "#06B6D4",
        subCategories: ["Training Data", "Financial Data", "Market Research", "User Behavior", "Geospatial Data", "Medical Records"],
        description: "Trade curated datasets for AI training, research, and analytics.",
    },
];

const goods: CategoryItem[] = [
    {
        id: "fashion", name: "Fashion", traders: "1k active traders", icon: "fashion",
        accentColor: "#EC4899",
        subCategories: ["Clothing", "Shoes & Footwear", "Accessories", "Bags & Luggage", "Jewelry", "Vintage & Luxury"],
        description: "Buy, sell or exchange clothing, accessories, and fashion items.",
    },
    {
        id: "electronics", name: "Electronics & Devices", traders: "1k active traders", icon: "laptop",
        accentColor: "#6366F1",
        subCategories: ["Smartphones", "Laptops & PCs", "Cameras", "Audio Equipment", "Gaming Gear", "Smart Home"],
        description: "Trade electronics, devices, and tech gadgets.",
    },
    {
        id: "books", name: "Books & Learning", traders: "1k active traders", icon: "book",
        accentColor: "#84CC16",
        subCategories: ["Fiction", "Non-Fiction", "Textbooks", "Magazines", "Comics & Manga", "Study Materials"],
        description: "Exchange books, learning materials, and educational resources.",
    },
    {
        id: "sports", name: "Sports & Travel", traders: "1k active traders", icon: "travel",
        accentColor: "#F97316",
        subCategories: ["Sports Equipment", "Outdoor Gear", "Travel Accessories", "Fitness", "Cycling", "Water Sports"],
        description: "Find sports equipment and travel gear for your adventures.",
    },
    {
        id: "toys", name: "Toys & Games", traders: "1k active traders", icon: "toys",
        accentColor: "#FBBF24",
        subCategories: [
            "Rubik's Cube & Speed Cubes",
            "Action Figures & Collectibles",
            "Board Games & Card Games",
            "LEGO & Building Blocks",
            "Remote Control Toys",
            "Puzzles",
            "Stuffed Animals & Plushies",
            "Educational Toys",
            "Outdoor & Sandbox Toys",
            "Diecast & Model Cars",
            "Trading Card Games (TCG)",
            "Anime & Manga Figures",
        ],
        description: "Mua bán trao đổi đồ chơi, trò chơi board game, figure, Rubik và collectibles.",
    },
    {
        id: "home", name: "Home & Living", traders: "1k active traders", icon: "home",
        accentColor: "#34D399",
        subCategories: [
            "Kitchen & Cooking",
            "Furniture & Decor",
            "Bedding & Pillows",
            "Bathroom Essentials",
            "Cleaning & Organizers",
            "Lighting",
            "Plants & Gardening",
            "Air Purifiers & Fans",
            "Rice Cookers & Small Appliances",
            "Storage & Shelving",
            "Wall Art & Frames",
            "Candles & Aromatherapy",
        ],
        description: "Đồ gia dụng, nội thất, trang trí nhà cửa và thiết bị nhà bếp.",
    },
];

const freelanceJobs: FreelanceItem[] = [
    {
        id: "tech",
        name: "Tech & IT",
        jobs: "10 jobs",
        icon: "tech",
        accentColor: "#F15CDD",
        description: "Software development, data science, IT support and more.",
        details: [
            "Web & Mobile Development",
            "Software / App Development",
            "Data Science / Analytics",
            "IT Support / Networking",
        ],
    },
    {
        id: "design",
        name: "Design & Creative",
        jobs: "10 jobs",
        icon: "design",
        accentColor: "#47E069",
        description: "Graphic design, UI/UX, illustration, and creative work.",
        details: [
            "Graphic Design",
            "UI/UX Design",
            "Illustration / Animation",
            "Video & Photo Editing",
        ],
    },
    {
        id: "writing",
        name: "Writing & Translation",
        jobs: "10 jobs",
        icon: "translate",
        accentColor: "#4DA6FF",
        description: "Content writing, copywriting, translation, and documentation.",
        details: [
            "Content Writing / Copywriting",
            "Blogging / Articles",
            "Translation / Localization",
            "Technical Writing",
        ],
    },
    {
        id: "marketing",
        name: "Marketing & Sales",
        jobs: "10 jobs",
        icon: "marketing",
        accentColor: "#F59E0B",
        description: "Digital marketing, social media, SEO, and sales strategy.",
        details: [
            "Digital Marketing",
            "Social Media Management",
            "SEO / SEM",
            "Sales & Lead Generation",
        ],
    },
];



// Lookup: sub_category string → { selectedCategory, subtopic }
// Used to auto-navigate Discover tab when AI classifies intent
function findCategoryBySubCategory(subCategory: string): { selected: SelectedCategory; subtopic: string } | null {
    const allGoods = goods;
    const allDigital = digitalAssets;
    for (const item of allGoods) {
        if (item.subCategories.includes(subCategory)) {
            return { selected: { type: 'goods', item }, subtopic: subCategory };
        }
    }
    for (const item of allDigital) {
        if (item.subCategories.includes(subCategory)) {
            return { selected: { type: 'digital', item }, subtopic: subCategory };
        }
    }
    return null;
}

// Icon mapping
const iconMap: Record<string, React.ReactNode> = {
    video: <img src={videoIcon} alt="Video" />,
    image: <img src={imagesIcon} alt="Images" />,
    audio: <img src={audioIcon} alt="Audio" />,
    token: <img src={tokenIcon} alt="Token" />,
    key: <img src={keyIcon} alt="Key" />,
    document: <img src={documentIcon} alt="Document" />,
    code: <img src={sourceCodeIcon} alt="Source Code" />,
    data: <img src={datasetIcon} alt="Dataset" />,
    fashion: <img src={fashionIcon} alt="Fashion" />,
    laptop: <img src={electronicsIcon} alt="Electronics" />,
    book: <img src={booksIcon} alt="Books" />,
    travel: <img src={sportsIcon} alt="Sports & Travel" />,
    tech: <img src={techIcon} alt="Tech" />,
    design: <img src={designIcon} alt="Design" />,
    translate: <img src={translateIcon} alt="Translation" />,
    marketing: <img src={marketingIcon} alt="Marketing" />,
    // Inline SVG icons for new goods categories
    toys: (
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.6" strokeLinecap="round" strokeLinejoin="round">
            {/* Rubik's cube icon */}
            <rect x="2" y="7" width="10" height="10" rx="1.5"/>
            <rect x="12" y="2" width="10" height="10" rx="1.5"/>
            <rect x="12" y="12" width="10" height="10" rx="1.5"/>
            <line x1="2" y1="10" x2="12" y2="10"/>
            <line x1="2" y1="13" x2="12" y2="13"/>
            <line x1="15" y1="2" x2="15" y2="12"/>
            <line x1="18" y1="2" x2="18" y2="12"/>
            <line x1="15" y1="15" x2="22" y2="15"/>
            <line x1="15" y1="18" x2="22" y2="18"/>
            <line x1="5" y1="7" x2="5" y2="17"/>
            <line x1="8" y1="7" x2="8" y2="17"/>
        </svg>
    ),
    home: (
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.6" strokeLinecap="round" strokeLinejoin="round">
            <path d="M3 9.5L12 3l9 6.5V20a1 1 0 01-1 1H4a1 1 0 01-1-1V9.5z"/>
            <path d="M9 21V12h6v9"/>
        </svg>
    ),
};

// Category Detail View
function CategoryDetailView({
    selected,
    onBack,
    onSubtopicSelect,
}: {
    selected: SelectedCategory;
    onBack: () => void;
    onSubtopicSelect: (sub: string) => void;
}) {
    const item = selected.item;
    const isFreelance = selected.type === 'freelance';
    const freelanceItem = isFreelance ? (item as FreelanceItem) : null;
    const categoryItem = !isFreelance ? (item as CategoryItem) : null;
    const accentColor = item.accentColor;
    const subItems = isFreelance ? freelanceItem!.details : categoryItem!.subCategories;
    const countLabel = isFreelance ? freelanceItem!.jobs : categoryItem!.traders;

    return (
        <div className="category-detail-view">
            {/* Back Button */}
            <button className="detail-back-btn" onClick={onBack}>
                <svg width="16" height="16" viewBox="0 0 16 16" fill="none">
                    <path d="M10 12L6 8L10 4" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round"/>
                </svg>
                Back
            </button>

            {/* Hero Header */}
            <div className="detail-hero" style={{ '--accent': accentColor } as React.CSSProperties}>
                <div className="detail-hero-icon" style={{ background: `${accentColor}20`, border: `1.5px solid ${accentColor}40` }}>
                    <span style={{ filter: `drop-shadow(0 0 8px ${accentColor}80)` }}>
                        {iconMap[item.icon]}
                    </span>
                </div>
                <div className="detail-hero-info">
                    <h2 className="detail-hero-title">{item.name}</h2>
                    <p className="detail-hero-desc">{item.description}</p>
                    <span className="detail-hero-badge" style={{ background: `${accentColor}18`, color: accentColor, border: `1px solid ${accentColor}35` }}>
                        {countLabel}
                    </span>
                </div>
            </div>

            {/* Sub-categories / Details */}
            <div className="detail-section">
                <h3 className="detail-section-title">{isFreelance ? "Specializations" : "Sub-categories"}</h3>
                <div className="detail-sub-grid">
                    {subItems.map((sub, i) => (
                        <div
                            key={i}
                            className="detail-sub-card"
                            style={{ '--accent': accentColor } as React.CSSProperties}
                            onClick={() => onSubtopicSelect(sub)}
                        >
                            <span className="detail-sub-dot" style={{ background: accentColor }} />
                            <span className="detail-sub-name">{sub}</span>
                            <svg className="detail-sub-arrow" width="14" height="14" viewBox="0 0 14 14" fill="none">
                                <path d="M5 3L9 7L5 11" stroke="currentColor"
                                    strokeWidth="1.4" strokeLinecap="round" strokeLinejoin="round"/>
                            </svg>
                        </div>
                    ))}
                </div>
            </div>

            {/* CTA */}
            <div className="detail-cta">
                <button
                    className="detail-cta-btn"
                    style={{ background: accentColor, boxShadow: `0 4px 24px ${accentColor}40` }}
                >
                    Post a Request
                </button>
                <button className="detail-cta-btn-ghost" style={{ borderColor: `${accentColor}50`, color: accentColor }}>
                    Browse Offers
                </button>
            </div>
        </div>
    );
}

export function MarketplacePage() {
    const [activeTab, setActiveTab] = useState<TabType>('discover');
    const activeTabRef = useRef<TabType>('discover');
    useEffect(() => { activeTabRef.current = activeTab; }, [activeTab]);
    const [selectedCategory, setSelectedCategory] = useState<SelectedCategory | null>(null);
    const [selectedSubtopic, setSelectedSubtopic] = useState<string | null>(null);
    const [lastAIAction, setLastAIAction] = useState<string | undefined>(undefined);
    const [formData, setFormData] = useState<Partial<DynamicFormFields> | null>(null);
    const formDataRef = useRef<Partial<DynamicFormFields> | null>(null);
    useEffect(() => { formDataRef.current = formData; }, [formData]);
    const [currentFormData, setCurrentFormData] = useState<Partial<DynamicFormFields> | null>(null);
    const [isFormStreaming, setIsFormStreaming] = useState(false);
    const [discoveryFormData, setDiscoveryFormData] = useState<DynamicFormFields | null>(null);
    const [negotiationCandidates, setNegotiationCandidates] = useState<Candidate[]>([]);
    const [activeOffers, setActiveOffers] = useState<ActiveOffer[]>([]);
    const activeOffersRef = useRef<ActiveOffer[]>([]);
    useEffect(() => { activeOffersRef.current = activeOffers; }, [activeOffers]);
    const [selectedOfferId, setSelectedOfferId] = useState<string | null>(null);
    // Stable refs map: offerId → MutableRefObject<stopFn>
    const offerStopRefs = useRef<Record<string, React.MutableRefObject<(() => void) | null>>>({});
    // Ref để dừng polling từ DiscoveryView khi deal xong (legacy, kept for compatibility)
    const stopDiscoveryRef = useRef<(() => void) | null>(null);

    // Listen for form data from AI chat (via custom event or context)
    useEffect(() => {
        const handleFormData = (event: CustomEvent<{ data: Partial<DynamicFormFields>, streaming?: boolean }>) => {
            console.log("📋 [Marketplace] Received form data:", event.detail);
            setFormData(event.detail.data);
            setIsFormStreaming(event.detail.streaming || false);
            // Lưu action từ AI để truyền xuống SubtopicDashboard khi announce tracker
            if (event.detail.data.action) {
                setLastAIAction(event.detail.data.action);
            }
            // Nếu AI đã xác định được sub_category, tự động navigate Discover tab đến đúng vị trí
            const subCat = (event.detail.data.selection as any)?.subtopic;
            if (subCat) {
                const found = findCategoryBySubCategory(subCat);
                if (found) {
                    setSelectedCategory(found.selected);
                    setSelectedSubtopic(found.subtopic);
                }
            }
            if (activeTabRef.current !== 'finding' && activeTabRef.current !== 'negotiation') {
                setActiveTab('create'); // Auto switch to create tab
            }
        };

        // Listen for start discovery event (from DynamicRequestForm submit or StartTransactionButton)
        const handleStartDiscovery = (e: Event) => {
            const data = (e as CustomEvent<DynamicFormFields>).detail
                || formDataRef.current as DynamicFormFields;
            if (!data?.topic) return;

            // Deduplicate: if offer already running for same topic+action, just navigate
            const existing = activeOffersRef.current.find(o =>
                o.formData.topic === data.topic && o.formData.action === data.action
            );
            if (existing) {
                setActiveTab('finding');
                return;
            }
            console.log("🔍 [Marketplace] Starting discovery with form data:", data);
            const offerId = Date.now().toString();
            // Create stable ref for this offer
            offerStopRefs.current[offerId] = { current: null };
            const newOffer: ActiveOffer = {
                id: offerId,
                formData: data,
                candidates: [],
                createdAt: Date.now(),
                status: 'searching',
            };
            setActiveOffers(prev => [...prev, newOffer]);
            setFormData(null);
            setActiveTab('finding');
            const subtopic = data.selection && 'subtopic' in data.selection
                ? (data.selection as any).subtopic
                : data.selection && 'category' in data.selection
                    ? (data.selection as any).category
                    : undefined;
            getMyNodeId().then((nodeId: string) => {
                if (nodeId) announceTopic(data.topic, nodeId, subtopic, data.action);
            }).catch(() => {});
        };

        window.addEventListener('k2:showDynamicForm' as any, handleFormData);
        window.addEventListener('k2:startDiscovery' as any, handleStartDiscovery);
        return () => {
            window.removeEventListener('k2:showDynamicForm' as any, handleFormData);
            window.removeEventListener('k2:startDiscovery' as any, handleStartDiscovery);
        };
    }, []);  // No deps needed — handleStartDiscovery reads from event.detail

    const [broadcastStatus, setBroadcastStatus] = useState<"idle" | "broadcasting" | "success" | "error">("idle");
    const [broadcastMessage, setBroadcastMessage] = useState("");

    const handleFormSubmit = useCallback(async (data: DynamicFormFields) => {
        console.log("📤 [Marketplace] Form submitted:", data);
        setBroadcastStatus("broadcasting");
        setBroadcastMessage("");

        try {
            const result = await apiPostOffer(data.topic, data.action, data);
            const offerId = result.offer_id;
            console.log("✅ [Marketplace] Post offer OK:", offerId, result.status);
            setBroadcastStatus("success");
            if (result.status === "matched") {
                setBroadcastMessage(`Đã tìm thấy match! ID: ${offerId}`);
            } else {
                setBroadcastMessage(`Đã đăng lên server! ID: ${offerId}`);
            }
            window.dispatchEvent(new CustomEvent('k2:formSubmitted', { detail: { ...data, offerId } }));
            // Announce lên tracker với action + subtopic để dashboard cập nhật live
            const subtopic = data.selection && 'subtopic' in data.selection
                ? data.selection.subtopic
                : data.selection && 'category' in data.selection
                    ? data.selection.category
                    : undefined;
            getMyNodeId().then((nodeId: string) => {
                if (nodeId) announceTopic(data.topic, nodeId, subtopic, data.action);
            }).catch(() => {});
            setTimeout(() => setBroadcastStatus("idle"), 4000);
        } catch (err) {
            console.error("❌ [Marketplace] Post offer failed:", err);
            setBroadcastStatus("error");
            setBroadcastMessage(String(err));
            setTimeout(() => setBroadcastStatus("idle"), 4000);
        }
    }, []);

    const handleFormCancel = () => {
        setFormData(null);
        setActiveTab('discover');
    };

    // Handle start negotiation from DiscoveryView
    const handleStartNegotiation = (candidates: Candidate[]) => {
        console.log("🤝 [Marketplace] Starting negotiation with candidates:", candidates);
        setNegotiationCandidates(candidates);
        setActiveTab('negotiation');
    };

    // Handle negotiation complete
    const handleNegotiationComplete = (results: Candidate[]) => {
        console.log("✅ [Marketplace] Negotiation complete:", results);
        // Event already dispatched by NegotiationDashboard
    };

    return (
        <div className="marketplace-content" style={{ position: 'relative' }}>
            {/* Broadcast Status Toast */}
            {broadcastStatus !== "idle" && (
                <div style={{
                    position: 'fixed', bottom: 24, right: 24, zIndex: 1000,
                    background: broadcastStatus === "success" ? "#1a2e1a" : broadcastStatus === "error" ? "#2e1a1a" : "#1a1a2e",
                    border: `1px solid ${broadcastStatus === "success" ? "#47E069" : broadcastStatus === "error" ? "#FF6B6B" : "#4DA6FF"}`,
                    borderRadius: 10, padding: "12px 18px", maxWidth: 320,
                    color: broadcastStatus === "success" ? "#47E069" : broadcastStatus === "error" ? "#FF6B6B" : "#4DA6FF",
                    fontSize: 13, fontWeight: 500,
                }}>
                    {broadcastStatus === "broadcasting" && "📡 Đang broadcast lên mạng P2P..."}
                    {broadcastStatus === "success" && `✅ ${broadcastMessage}`}
                    {broadcastStatus === "error" && `❌ Lỗi: ${broadcastMessage}`}
                </div>
            )}
            {/* Tab Navigation */}
            <MarketplaceTabs activeTab={activeTab} onTabChange={setActiveTab} activeOffersCount={activeOffers.length} />

            {/* Always-mounted DiscoveryViews — polling continues regardless of active tab */}
            <div style={{ display: 'none' }}>
                {activeOffers.map(offer => (
                    <DiscoveryView
                        key={offer.id}
                        formData={offer.formData}
                        onMatchFound={(_count, candidates) => {
                            setActiveOffers(prev => prev.map(o =>
                                o.id === offer.id
                                    ? { ...o, candidates: [...candidates].sort((a, b) => (b.matchScore ?? 0) - (a.matchScore ?? 0)), status: candidates.length > 0 ? 'found' : 'searching' }
                                    : o
                            ));
                        }}
                        onStartNegotiation={(candidates) => {
                            setSelectedOfferId(offer.id);
                            setDiscoveryFormData(offer.formData);
                            setNegotiationCandidates(candidates);
                            setActiveTab('negotiation');
                        }}
                        onCancel={() => {}}
                        stopPollingRef={offerStopRefs.current[offer.id]}
                    />
                ))}
            </div>

            {/* Tab Content */}
            {activeTab === 'discover' ? (
                // Screen 3: Subtopic dashboard (full page)
                selectedCategory && selectedSubtopic ? (
                    <SubtopicDashboard
                        topic={selectedCategory.item.name}
                        subtopic={selectedSubtopic}
                        accentColor={selectedCategory.item.accentColor}
                        onClose={() => setSelectedSubtopic(null)}
                        action={lastAIAction}
                    />
                ) : selectedCategory ? (
                // Screen 2: Category detail
                    <CategoryDetailView
                        selected={selectedCategory}
                        onBack={() => { setSelectedCategory(null); setSelectedSubtopic(null); }}
                        onSubtopicSelect={(sub) => setSelectedSubtopic(sub)}
                    />
                ) : (
                // Screen 1: Category list
                <>
                    <h2 className="discover-title">Discover Deals</h2>

                    {/* Digital Assets Section */}
                    <section className="category-section">
                        <h3 className="section-label">Digital Assets</h3>
                        <div className="category-grid">
                            {digitalAssets.map((item) => (
                                <div
                                    key={item.id}
                                    className="category-card"
                                    style={{ '--card-accent': item.accentColor } as React.CSSProperties}
                                    onClick={() => { setSelectedSubtopic(null); setSelectedCategory({ type: 'digital', item }); }}
                                >
                                    <div className="card-icon-wrap" style={{ background: `${item.accentColor}18`, border: `1px solid ${item.accentColor}30` }}>
                                        <div className="card-icon">{iconMap[item.icon]}</div>
                                    </div>
                                    <div className="card-info">
                                        <span className="card-name">{item.name}</span>
                                        <span className="card-traders">{item.traders}</span>
                                    </div>
                                    <svg className="card-chevron" width="16" height="16" viewBox="0 0 16 16" fill="none">
                                        <path d="M6 4L10 8L6 12" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round"/>
                                    </svg>
                                </div>
                            ))}
                        </div>
                    </section>

                    {/* Goods Section */}
                    <section className="category-section">
                        <h3 className="section-label">Goods</h3>
                        <div className="category-grid">
                            {goods.map((item) => (
                                <div
                                    key={item.id}
                                    className="category-card"
                                    style={{ '--card-accent': item.accentColor } as React.CSSProperties}
                                    onClick={() => { setSelectedSubtopic(null); setSelectedCategory({ type: 'goods', item }); }}
                                >
                                    <div className="card-icon-wrap" style={{ background: `${item.accentColor}18`, border: `1px solid ${item.accentColor}30` }}>
                                        <div className="card-icon">{iconMap[item.icon]}</div>
                                    </div>
                                    <div className="card-info">
                                        <span className="card-name">{item.name}</span>
                                        <span className="card-traders">{item.traders}</span>
                                    </div>
                                    <svg className="card-chevron" width="16" height="16" viewBox="0 0 16 16" fill="none">
                                        <path d="M6 4L10 8L6 12" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round"/>
                                    </svg>
                                </div>
                            ))}
                        </div>
                    </section>

                    {/* Freelance Job Section */}
                    <section className="category-section">
                        <h3 className="section-label">Freelance Job</h3>
                        <div className="job-grid">
                            {freelanceJobs.map((job) => (
                                <div
                                    key={job.id}
                                    className="job-card"
                                    style={{ '--card-accent': job.accentColor } as React.CSSProperties}
                                    onClick={() => { setSelectedSubtopic(null); setSelectedCategory({ type: 'freelance', item: job }); }}
                                >
                                    <div className="job-header">
                                        <div className="job-icon-wrap" style={{ background: `${job.accentColor}18`, border: `1px solid ${job.accentColor}30` }}>
                                            <span className="job-icon">{iconMap[job.icon]}</span>
                                        </div>
                                        <span className="job-name">{job.name}</span>
                                    </div>
                                    <ul className="job-details">
                                        {job.details.map((detail, idx) => (
                                            <li key={idx} style={{ '--bullet-color': job.accentColor } as React.CSSProperties}>{detail}</li>
                                        ))}
                                    </ul>
                                    <div className="job-footer">
                                        <span className="job-count" style={{ color: job.accentColor }}>{job.jobs}</span>
                                        <svg width="16" height="16" viewBox="0 0 16 16" fill="none">
                                            <path d="M6 4L10 8L6 12" stroke={job.accentColor} strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round"/>
                                        </svg>
                                    </div>
                                </div>
                            ))}
                        </div>
                    </section>
                </>
                ) // end screen 1
            ) : activeTab === 'create' ? (
                /* Create Request Tab */
                <div className="create-request-tab">
                    {isFormStreaming && !formData ? (
                        <SkeletonForm />
                    ) : formData ? (
                        <DynamicRequestForm
                            initialData={formData}
                            onSubmit={handleFormSubmit}
                            onCancel={handleFormCancel}
                            onFormChange={setCurrentFormData}
                            isStreaming={isFormStreaming}
                        />
                    ) : (
                        <div className="empty-form-state">
                            {/* Empty state when no form yet */}
                        </div>
                    )}
                </div>
            ) : (
                /* Finding + Negotiation */
                <>
                    {/* Finding Match Tab */}
                    <div className="finding-match-tab" style={{ display: activeTab === 'finding' ? undefined : 'none' }}>
                        {/* UI: list of active offers */}
                        <FindMatchingView
                            offers={activeOffers}
                            onStartNegotiation={(offerId) => {
                                const offer = activeOffers.find(o => o.id === offerId);
                                if (!offer) return;
                                setSelectedOfferId(offerId);
                                setDiscoveryFormData(offer.formData);
                                setNegotiationCandidates(offer.candidates);
                                setActiveTab('negotiation');
                            }}
                            onCancelOffer={(offerId) => {
                                offerStopRefs.current[offerId]?.current?.();
                                delete offerStopRefs.current[offerId];
                                const offer = activeOffers.find(o => o.id === offerId);
                                if (offer?.formData?.topic) {
                                    apiCancelOffer(offer.formData.topic).catch(() => {});
                                }
                                setActiveOffers(prev => prev.filter(o => o.id !== offerId));
                            }}
                        />
                    </div>

                    {/* Negotiation Tab */}
                    {activeTab === 'negotiation' && (
                        <div className="negotiation-tab">
                            <NegotiationDashboard
                                candidates={negotiationCandidates}
                                formData={discoveryFormData}
                                onComplete={handleNegotiationComplete}
                                onBack={() => {
                                    setActiveTab('finding');
                                }}
                                onDealDone={() => {
                                    // Dừng polling của offer đang deal
                                    if (selectedOfferId) {
                                        offerStopRefs.current[selectedOfferId]?.current?.();
                                        delete offerStopRefs.current[selectedOfferId];
                                        const offer = activeOffers.find(o => o.id === selectedOfferId);
                                        if (offer?.formData?.topic) {
                                            const topic = offer.formData.topic;
                                            apiCancelOffer(topic).catch(() => {});
                                            getMyNodeId().then((nodeId: string) => {
                                                if (nodeId) apiLeaveTopic(topic, nodeId).catch(() => {});
                                            }).catch(() => {});
                                        }
                                        // Chỉ xóa offer đang deal, không xóa các offer khác
                                        setActiveOffers(prev => {
                                            const remaining = prev.filter(o => o.id !== selectedOfferId);
                                            // Quay về finding nếu còn offer, về discover nếu hết
                                            setActiveTab(remaining.length > 0 ? 'finding' : 'discover');
                                            return remaining;
                                        });
                                        setSelectedOfferId(null);
                                    } else {
                                        setActiveTab('discover');
                                    }
                                    setDiscoveryFormData(null);
                                    setNegotiationCandidates([]);
                                    setBroadcastStatus('success');
                                    setBroadcastMessage('Giao dịch đã hoàn tất!');
                                    console.log("✅ [Marketplace] Deal confirmed");
                                }}
                            />
                        </div>
                    )}
                </>
            )}
        </div>
    );
}
