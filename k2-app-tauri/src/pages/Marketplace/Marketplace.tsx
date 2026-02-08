import { useState, useEffect } from "react";
import "./Marketplace.css";
import { MarketplaceTabs, DynamicRequestForm, SkeletonForm, DiscoveryView, NegotiationDashboard } from "../../components/DynamicForm";
import type { TabType, Candidate } from "../../components/DynamicForm";
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

// Category data based on Figma design
const digitalAssets = [
    { id: "video", name: "Video", traders: "1k active traders", icon: "video" },
    { id: "images", name: "Images", traders: "1k active traders", icon: "image" },
    { id: "audio", name: "Audio", traders: "1k active traders", icon: "audio" },
    { id: "token", name: "Token", traders: "1k active traders", icon: "token" },
    { id: "license", name: "License | Key | Secret", traders: "1k active traders", icon: "key" },
    { id: "document", name: "Document", traders: "1k active traders", icon: "document" },
    { id: "source-code", name: "Source Code", traders: "1k active traders", icon: "code" },
    { id: "dataset", name: "Dataset", traders: "1k active traders", icon: "data" },
];

const goods = [
    { id: "fashion", name: "Fashion", traders: "1k active traders", icon: "fashion" },
    { id: "electronics", name: "Electronics & Devices", traders: "1k active traders", icon: "laptop" },
    { id: "books", name: "Books & Learning", traders: "1k active traders", icon: "book" },
    { id: "sports", name: "Sports & Travel", traders: "1k active traders", icon: "travel" },
];

const freelanceJobs = [
    {
        id: "tech",
        name: "Tech & IT",
        jobs: "10 jobs",
        icon: "tech",
        color: "#F15CDD",
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
        color: "#47E069",
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
        color: "#FFFFFF",
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
        color: "#FFFFFF",
        details: [
            "Digital Marketing",
            "Social Media Management",
            "SEO / SEM",
            "Sales & Lead Generation",
        ],
    },
];

// Icon components






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
};

export function MarketplacePage() {
    const [activeTab, setActiveTab] = useState<TabType>('discover');
    const [formData, setFormData] = useState<Partial<DynamicFormFields> | null>(null);
    const [currentFormData, setCurrentFormData] = useState<Partial<DynamicFormFields> | null>(null); // Live form data from DynamicRequestForm
    const [isFormStreaming, setIsFormStreaming] = useState(false);
    const [discoveryFormData, setDiscoveryFormData] = useState<DynamicFormFields | null>(null);
    const [negotiationCandidates, setNegotiationCandidates] = useState<Candidate[]>([]);

    // Listen for form data from AI chat (via custom event or context)
    useEffect(() => {
        const handleFormData = (event: CustomEvent<{ data: Partial<DynamicFormFields>, streaming?: boolean }>) => {
            console.log("📋 [Marketplace] Received form data:", event.detail);
            setFormData(event.detail.data);
            setIsFormStreaming(event.detail.streaming || false);
            setActiveTab('create'); // Auto switch to create tab
        };

        // Listen for start discovery event (from chat "Bắt đầu giao dịch" button)
        const handleStartDiscovery = () => {
            // Use currentFormData from DynamicRequestForm (has priceRange and all user edits)
            console.log("🔍 [Marketplace] Starting discovery with form data:", currentFormData);
            setDiscoveryFormData(currentFormData as DynamicFormFields);
            setActiveTab('finding'); // Switch to finding tab
        };

        window.addEventListener('k2:showDynamicForm' as any, handleFormData);
        window.addEventListener('k2:startDiscovery' as any, handleStartDiscovery);
        return () => {
            window.removeEventListener('k2:showDynamicForm' as any, handleFormData);
            window.removeEventListener('k2:startDiscovery' as any, handleStartDiscovery);
        };
    }, [currentFormData]);  // Depend on currentFormData to get latest value

    const handleFormSubmit = (data: DynamicFormFields) => {
        console.log("📤 [Marketplace] Form submitted:", data);
        // TODO: Dispatch to P2P network
        // For now, emit event for chat to handle
        window.dispatchEvent(new CustomEvent('k2:formSubmitted', { detail: data }));
    };

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
        <div className="marketplace-content">
            {/* Tab Navigation */}
            <MarketplaceTabs activeTab={activeTab} onTabChange={setActiveTab} />

            {/* Tab Content */}
            {activeTab === 'discover' ? (
                <>
                    <h2 className="discover-title">Discover Deals</h2>

                    {/* Digital Assets Section */}
                    <section className="category-section">
                        <h3 className="section-label">Digital Assets</h3>
                        <div className="category-grid">
                            {digitalAssets.map((item) => (
                                <div key={item.id} className="category-card">
                                    <div className="card-icon">{iconMap[item.icon]}</div>
                                    <div className="card-info">
                                        <span className="card-name">{item.name}</span>
                                        <span className="card-traders">{item.traders}</span>
                                    </div>
                                </div>
                            ))}
                        </div>
                    </section>

                    {/* Goods Section */}
                    <section className="category-section">
                        <h3 className="section-label">Goods</h3>
                        <div className="category-grid">
                            {goods.map((item) => (
                                <div key={item.id} className="category-card">
                                    <div className="card-icon">{iconMap[item.icon]}</div>
                                    <div className="card-info">
                                        <span className="card-name">{item.name}</span>
                                        <span className="card-traders">{item.traders}</span>
                                    </div>
                                </div>
                            ))}
                        </div>
                    </section>

                    {/* Freelance Job Section */}
                    <section className="category-section">
                        <h3 className="section-label">Freelance Job</h3>
                        <div className="job-grid">
                            {freelanceJobs.map((job) => (
                                <div key={job.id} className="job-card">
                                    <div className="job-header">
                                        <span className="job-icon" style={{ color: job.color }}>
                                            {iconMap[job.icon]}
                                        </span>
                                        <span className="job-name">{job.name}</span>
                                    </div>
                                    <ul className="job-details">
                                        {job.details.map((detail, idx) => (
                                            <li key={idx}>{detail}</li>
                                        ))}
                                    </ul>
                                    <span className="job-count">{job.jobs}</span>
                                </div>
                            ))}
                        </div>
                    </section>
                </>
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
            ) : activeTab === 'finding' ? (
                /* Finding Match Tab */
                <div className="finding-match-tab">
                    <DiscoveryView
                        formData={discoveryFormData}
                        onMatchFound={(count, candidates) => {
                            console.log(`🎯 [Marketplace] Found ${count} matches`);
                            // Store candidates for potential negotiation
                            if (candidates && candidates.length > 0) {
                                setNegotiationCandidates(candidates);
                            }
                        }}
                        onStartNegotiation={handleStartNegotiation}
                        onCancel={() => {
                            setActiveTab('create');
                        }}
                    />
                </div>
            ) : (
                /* Negotiation Tab */
                <div className="negotiation-tab">
                    <NegotiationDashboard
                        candidates={negotiationCandidates}
                        formData={discoveryFormData}
                        onComplete={handleNegotiationComplete}
                        onBack={() => {
                            setActiveTab('finding');
                        }}
                    />
                </div>
            )}
        </div>
    );
}
