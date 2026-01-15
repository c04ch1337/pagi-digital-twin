import React from 'react';

interface ButtonProps extends React.ButtonHTMLAttributes<HTMLButtonElement> {
  variant?: 'default' | 'outline' | 'destructive' | 'secondary' | 'ghost';
  size?: 'sm' | 'md' | 'lg';
  children: React.ReactNode;
}

export function Button({ 
  variant = 'default', 
  size = 'md', 
  children, 
  className = '', 
  ...props 
}: ButtonProps) {
  const baseClasses = 'inline-flex items-center justify-center rounded-md font-medium transition-colors focus:outline-none focus:ring-2 focus:ring-offset-2 disabled:opacity-50 disabled:pointer-events-none';
  
  const variantClasses = {
    default: 'bg-[var(--bg-steel)] text-[var(--text-primary)] hover:bg-[rgb(var(--bg-steel-rgb)/0.8)]',
    outline: 'border border-[rgb(var(--bg-steel-rgb)/0.3)] bg-transparent hover:bg-[rgb(var(--surface-rgb)/0.4)]',
    destructive: 'bg-[rgb(var(--danger-rgb))] text-white hover:bg-[rgb(var(--danger-rgb)/0.8)]',
    secondary: 'bg-[rgb(var(--surface-rgb)/0.4)] text-[var(--text-secondary)] hover:bg-[rgb(var(--surface-rgb)/0.6)]',
    ghost: 'hover:bg-[rgb(var(--surface-rgb)/0.4)]',
  };
  
  const sizeClasses = {
    sm: 'h-8 px-3 text-sm',
    md: 'h-10 px-4 py-2',
    lg: 'h-12 px-6 text-lg',
  };
  
  return (
    <button
      className={`${baseClasses} ${variantClasses[variant]} ${sizeClasses[size]} ${className}`}
      {...props}
    >
      {children}
    </button>
  );
}
