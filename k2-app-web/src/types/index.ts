export type TabType = "marketplace" | "negotiation" | "contact" | "profile";

export interface Contact {
    node_id: string;
    nickname: string;
    added_at: number;
    notes: string | null;
}

export const TAB_LABELS: Record<TabType, string> = {
    marketplace: "Marketplace",
    negotiation: "Negotiation",
    contact: "Contact",
    profile: "Profile",
};
