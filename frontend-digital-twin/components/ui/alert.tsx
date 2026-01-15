import React from 'react';

interface AlertProps {
  children: React.ReactNode;
  className?: string;
}

export function Alert({ children, className = '' }: AlertProps) {
  return (
    <div className={`relative flex items-start gap-3 rounded-lg border p-4 ${className}`}>
      {children}
    </div>
  );
}

export function AlertDescription({ children, className = '' }: AlertProps) {
  return (
    <div className={`text-sm ${className}`}>
      {children}
    </div>
  );
}
