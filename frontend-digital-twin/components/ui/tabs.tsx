import React, { createContext, useContext } from 'react';

interface TabsContextType {
  value: string;
  onValueChange: (value: string) => void;
}

const TabsContext = createContext<TabsContextType | null>(null);

interface TabsProps {
  value: string;
  onValueChange: (value: string) => void;
  children: React.ReactNode;
  className?: string;
}

export function Tabs({ value, onValueChange, children, className = '' }: TabsProps) {
  return (
    <TabsContext.Provider value={{ value, onValueChange }}>
      <div className={className}>
        {children}
      </div>
    </TabsContext.Provider>
  );
}

export function TabsList({ children, className = '' }: { children: React.ReactNode; className?: string }) {
  return (
    <div className={`inline-flex h-10 items-center justify-center rounded-md bg-[rgb(var(--surface-rgb)/0.4)] p-1 ${className}`}>
      {children}
    </div>
  );
}

export function TabsTrigger({ 
  value, 
  children, 
  className = '' 
}: { 
  value: string; 
  children: React.ReactNode; 
  className?: string;
}) {
  const context = useContext(TabsContext);
  if (!context) throw new Error('TabsTrigger must be used within Tabs');
  
  const isActive = context.value === value;
  
  return (
    <button
      onClick={() => context.onValueChange(value)}
      className={`inline-flex items-center justify-center whitespace-nowrap rounded-sm px-3 py-1.5 text-sm font-medium transition-all focus-outline-none disabled:pointer-events-none disabled:opacity-50 ${
        isActive
          ? 'bg-[var(--bg-steel)] text-[var(--text-primary)]'
          : 'text-[var(--text-secondary)] hover:bg-[rgb(var(--surface-rgb)/0.6)]'
      } ${className}`}
    >
      {children}
    </button>
  );
}

export function TabsContent({ 
  value, 
  children, 
  className = '' 
}: { 
  value: string; 
  children: React.ReactNode; 
  className?: string;
}) {
  const context = useContext(TabsContext);
  if (!context) throw new Error('TabsContent must be used within Tabs');
  
  if (context.value !== value) return null;
  
  return (
    <div className={className}>
      {children}
    </div>
  );
}
