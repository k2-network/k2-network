import { NegotiationChat } from "./NegotiationChat";
import "./Negotiation.css";

interface NegotiationPageProps {
    openChatWith?: { nodeId: string; name: string } | null;
    onChatOpened?: () => void;
}

export function NegotiationPage({ openChatWith, onChatOpened }: NegotiationPageProps) {
    return <NegotiationChat openChatWith={openChatWith} onChatOpened={onChatOpened} />;
}
