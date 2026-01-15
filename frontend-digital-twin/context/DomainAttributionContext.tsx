import React, { createContext, useContext, useState, useCallback, useMemo } from 'react';
import { DomainAttribution } from '../types/protocol';

interface SessionAverage {
  mind: number;
  body: number;
  heart: number;
  soul: number;
  messageCount: number;
}

interface KnowledgeBaseStats {
  mind: number;
  body: number;
  heart: number;
  soul: number;
}

interface DomainAttributionContextType {
  currentAttribution: DomainAttribution | null;
  messageAttributions: Map<string, DomainAttribution>;
  sessionAverage: SessionAverage | null;
  knowledgeBaseStats: KnowledgeBaseStats;
  updateAttribution: (attribution: DomainAttribution | null, messageId?: string) => void;
  getAttributionForMessage: (messageId: string) => DomainAttribution | null;
  clearAttribution: () => void;
  getDominantDomain: (attribution: DomainAttribution | null) => 'M' | 'B' | 'H' | 'S' | null;
  getDomainDrift: () => 'balanced' | 'technical' | 'reactive' | 'personal' | 'ethical' | null;
  incrementKnowledgeBase: (domain: 'Mind' | 'Body' | 'Heart' | 'Soul') => void;
}

const DomainAttributionContext = createContext<DomainAttributionContextType | undefined>(undefined);

export const DomainAttributionProvider: React.FC<{ children: React.ReactNode }> = ({ children }) => {
  const [currentAttribution, setCurrentAttribution] = useState<DomainAttribution | null>(null);
  const [messageAttributions, setMessageAttributions] = useState<Map<string, DomainAttribution>>(new Map());
  const [knowledgeBaseStats, setKnowledgeBaseStats] = useState<KnowledgeBaseStats>({
    mind: 0,
    body: 0,
    heart: 0,
    soul: 0,
  });

  const updateAttribution = useCallback((attribution: DomainAttribution | null, messageId?: string) => {
    setCurrentAttribution(attribution);
    if (attribution && messageId) {
      setMessageAttributions(prev => {
        const newMap = new Map(prev);
        newMap.set(messageId, attribution);
        return newMap;
      });
    }
  }, []);

  const getAttributionForMessage = useCallback((messageId: string): DomainAttribution | null => {
    return messageAttributions.get(messageId) || null;
  }, [messageAttributions]);

  const getDominantDomain = useCallback((attribution: DomainAttribution | null): 'M' | 'B' | 'H' | 'S' | null => {
    if (!attribution) return null;
    const max = Math.max(attribution.mind, attribution.body, attribution.heart, attribution.soul);
    if (max === attribution.soul) return 'S';
    if (max === attribution.mind) return 'M';
    if (max === attribution.body) return 'B';
    if (max === attribution.heart) return 'H';
    return null;
  }, []);

  // Calculate session average from all message attributions
  const sessionAverage = useMemo((): SessionAverage | null => {
    if (messageAttributions.size === 0) return null;
    
    let totalMind = 0;
    let totalBody = 0;
    let totalHeart = 0;
    let totalSoul = 0;
    let count = 0;

    messageAttributions.forEach((attribution) => {
      totalMind += attribution.mind;
      totalBody += attribution.body;
      totalHeart += attribution.heart;
      totalSoul += attribution.soul;
      count++;
    });

    return {
      mind: totalMind / count,
      body: totalBody / count,
      heart: totalHeart / count,
      soul: totalSoul / count,
      messageCount: count,
    };
  }, [messageAttributions]);

  // Determine domain drift based on session average
  const getDomainDrift = useCallback((): 'balanced' | 'technical' | 'reactive' | 'personal' | 'ethical' | null => {
    if (!sessionAverage) return null;
    
    const { mind, body, heart, soul } = sessionAverage;
    const max = Math.max(mind, body, heart, soul);
    const threshold = 40; // Threshold for "dominant" domain
    
    if (max < 30) return 'balanced'; // All domains relatively equal
    if (mind > threshold && mind > body + 10) return 'technical';
    if (body > threshold && body > mind + 10) return 'reactive';
    if (heart > threshold) return 'personal';
    if (soul > threshold) return 'ethical';
    
    return 'balanced';
  }, [sessionAverage]);

  const clearAttribution = useCallback(() => {
    setCurrentAttribution(null);
    setMessageAttributions(new Map());
  }, []);

  const incrementKnowledgeBase = useCallback((domain: 'Mind' | 'Body' | 'Heart' | 'Soul') => {
    setKnowledgeBaseStats(prev => ({
      ...prev,
      [domain.toLowerCase()]: prev[domain.toLowerCase() as keyof KnowledgeBaseStats] + 1,
    }));
  }, []);

  const value = useMemo(() => ({
    currentAttribution,
    messageAttributions,
    sessionAverage,
    knowledgeBaseStats,
    updateAttribution,
    getAttributionForMessage,
    getDominantDomain,
    getDomainDrift,
    clearAttribution,
    incrementKnowledgeBase,
  }), [currentAttribution, messageAttributions, sessionAverage, knowledgeBaseStats, updateAttribution, getAttributionForMessage, getDominantDomain, getDomainDrift, clearAttribution, incrementKnowledgeBase]);

  return (
    <DomainAttributionContext.Provider value={value}>
      {children}
    </DomainAttributionContext.Provider>
  );
};

export const useDomainAttribution = (): DomainAttributionContextType => {
  const context = useContext(DomainAttributionContext);
  if (context === undefined) {
    throw new Error('useDomainAttribution must be used within a DomainAttributionProvider');
  }
  return context;
};
