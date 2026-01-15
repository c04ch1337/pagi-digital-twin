import React from 'react';

interface BadgeProps extends React.HTMLAttributes<HTMLDivElement> {
  variant?: 'default' | 'secondary' | 'destructive' | 'outline';
  children: React.ReactNode;
}

export function Badge({ variant = 'default', children, className = '', ...props }: BadgeProps) {
  const baseClasses = 'inline-flex items-center rounded-full px-2.5 py-0.5 text-xs font-semibold transition-colors';
  
  const variantClasses = {
    default: 'bg-[var(--bg-steel)] text-[var(--text-primary)]',
    secondary: 'bg-[rgb(var(--surface-rgb)/0.4)] text-[var(--text-secondary)]',
    destructive: 'bg-[rgb(var(--danger-rgb))] text-white',
    outline: 'border border-[rgb(var(--bg-steel-rgb)/0.3)] bg-transparent',
  };
  
  return (
    <div className={`${baseClasses} ${variantClasses[variant]} ${className}`} {...props}>
      {children}
    </div>
  );
}
