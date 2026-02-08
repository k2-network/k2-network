/**
 * StartTransactionButton Component
 * 
 * A button component that appears in chat after form creation.
 * When clicked, triggers the discovery phase in Tab 3.
 */
import React from 'react';
import { IoRocketOutline } from 'react-icons/io5';
import './StartTransactionButton.css';

interface StartTransactionButtonProps {
  actionText: string;
  title: string;
}

export const StartTransactionButton: React.FC<StartTransactionButtonProps> = ({
  actionText,
  title,
}) => {
  const handleClick = () => {
    console.log('🚀 [StartTransactionButton] Starting transaction');

    // Dispatch event to Marketplace - it will use the current form data
    window.dispatchEvent(new CustomEvent('k2:startDiscovery'));
  };

  return (
    <div className="start-transaction-wrapper">
      <p className="transaction-summary">
        Đã tạo form <strong>{actionText}</strong> cho <strong>{title}</strong>
      </p>
      <button className="start-transaction-btn" onClick={handleClick}>
        <IoRocketOutline className="btn-icon" />
        <span>Bắt đầu đàm phán</span>
      </button>
    </div>
  );
};

export default StartTransactionButton;
