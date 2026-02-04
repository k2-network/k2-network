/**
 * StartTransactionButton Component
 * 
 * A button component that appears in chat after form creation.
 * When clicked, triggers the discovery phase in Tab 3.
 */
import React from 'react';
import { IoRocketOutline } from 'react-icons/io5';
import './StartTransactionButton.css';
import type { DynamicFormFields } from '../DynamicForm/types';

interface StartTransactionButtonProps {
  formData: Partial<DynamicFormFields>;
  actionText: string;
  title: string;
}

export const StartTransactionButton: React.FC<StartTransactionButtonProps> = ({
  formData,
  actionText,
  title,
}) => {
  const handleClick = () => {
    console.log('🚀 [StartTransactionButton] Starting transaction:', formData);
    
    // Dispatch event to Marketplace to switch to Tab 3 and start discovery
    window.dispatchEvent(new CustomEvent('k2:startDiscovery', {
      detail: { formData }
    }));
  };

  return (
    <div className="start-transaction-wrapper">
      <p className="transaction-summary">
        Đã tạo form <strong>{actionText}</strong> cho <strong>{title}</strong>
      </p>
      <button className="start-transaction-btn" onClick={handleClick}>
        <IoRocketOutline className="btn-icon" />
        <span>Bắt đầu giao dịch</span>
      </button>
    </div>
  );
};

export default StartTransactionButton;
