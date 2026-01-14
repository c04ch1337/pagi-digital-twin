#!/usr/bin/env python3
"""
Phoenix Strategic Intent Analyzer

Analyzes governance reports to provide AI-driven strategic recommendations
explaining the gap between human rationale and machine consensus.
"""

import json
import sys
import os
from typing import Dict, List, Optional
from datetime import datetime

# Try to import LLM libraries (fallback to simple analysis if not available)
try:
    import openai
    HAS_OPENAI = True
except ImportError:
    HAS_OPENAI = False

try:
    from anthropic import Anthropic
    HAS_ANTHROPIC = True
except ImportError:
    HAS_ANTHROPIC = False

try:
    import google.generativeai as genai
    HAS_GEMINI = True
except ImportError:
    HAS_GEMINI = False


def analyze_with_llm(
    rationale: str,
    conflict_profile: List[Dict],
    agent_id: str,
    commit_hash: str
) -> str:
    """
    Use an LLM to analyze the strategic gap between human rationale and peer consensus.
    
    Returns a strategic recommendation string.
    """
    # Build context about the conflict
    disapproving_nodes = [c for c in conflict_profile if not c.get('approved', False)]
    approving_nodes = [c for c in conflict_profile if c.get('approved', False)]
    
    avg_disapproval_score = (
        sum(c.get('compliance_score', 0) for c in disapproving_nodes) / len(disapproving_nodes)
        if disapproving_nodes else 0
    )
    
    # Construct analysis prompt
    prompt = f"""You are a Strategic Governance Analyst for the Phoenix AGI mesh network.

**Context:**
- Agent ID: {agent_id}
- Commit Hash: {commit_hash[:8]}
- Human Rationale: {rationale}

**Peer Consensus Results:**
- Total Nodes: {len(conflict_profile)}
- Approving Nodes: {len(approving_nodes)}
- Disapproving Nodes: {len(disapproving_nodes)}
- Average Compliance Score of Disapproving Nodes: {avg_disapproval_score:.1f}%

**Disapproving Node Details:**
{chr(10).join(f"- Node {c.get('node_id', 'unknown')}: Score {c.get('compliance_score', 0):.1f}% - Reason: Likely failed compliance filters (Privacy, Tone, Security, etc.)" for c in disapproving_nodes[:5])}

**Task:**
Analyze the strategic gap between the human's rationale and the mesh's rejection. Provide:
1. A brief explanation of why the mesh rejected this (identify likely compliance filter failures)
2. A strategic recommendation for how to align the mesh's filters with operational needs
3. Specific actionable steps (e.g., "Update Privacy Scrubber Rule #104 to allow temporary diagnostic IPs")

Format your response as a concise strategic recommendation (2-3 paragraphs max)."""

    # Try different LLM providers in order of preference
    api_key = os.getenv('OPENAI_API_KEY')
    if HAS_OPENAI and api_key:
        try:
            client = openai.OpenAI(api_key=api_key)
            response = client.chat.completions.create(
                model=os.getenv('OPENAI_MODEL', 'gpt-4'),
                messages=[
                    {"role": "system", "content": "You are a Strategic Governance Analyst specializing in AI mesh network oversight."},
                    {"role": "user", "content": prompt}
                ],
                temperature=0.7,
                max_tokens=500
            )
            return response.choices[0].message.content.strip()
        except Exception as e:
            print(f"OpenAI API error: {e}", file=sys.stderr)
    
    api_key = os.getenv('ANTHROPIC_API_KEY')
    if HAS_ANTHROPIC and api_key:
        try:
            client = Anthropic(api_key=api_key)
            response = client.messages.create(
                model=os.getenv('ANTHROPIC_MODEL', 'claude-3-5-sonnet-20241022'),
                max_tokens=500,
                messages=[
                    {"role": "user", "content": prompt}
                ]
            )
            return response.content[0].text.strip()
        except Exception as e:
            print(f"Anthropic API error: {e}", file=sys.stderr)
    
    api_key = os.getenv('GEMINI_API_KEY')
    if HAS_GEMINI and api_key:
        try:
            genai.configure(api_key=api_key)
            model = genai.GenerativeModel(os.getenv('GEMINI_MODEL', 'gemini-pro'))
            response = model.generate_content(prompt)
            return response.text.strip()
        except Exception as e:
            print(f"Gemini API error: {e}", file=sys.stderr)
    
    # Fallback: Rule-based analysis
    return generate_fallback_analysis(rationale, conflict_profile, agent_id)


def generate_fallback_analysis(
    rationale: str,
    conflict_profile: List[Dict],
    agent_id: str
) -> str:
    """
    Fallback rule-based analysis when LLM is not available.
    """
    disapproving_nodes = [c for c in conflict_profile if not c.get('approved', False)]
    
    if not disapproving_nodes:
        return "Strategic Alignment: No peer conflicts detected. The override was likely preemptive or for operational consistency."
    
    # Analyze rationale keywords
    rationale_lower = rationale.lower()
    
    recommendations = []
    
    if any(word in rationale_lower for word in ['troubleshoot', 'diagnostic', 'debug', 'test']):
        recommendations.append(
            "Strategic Alignment: Human intervention was necessary for operational recovery or diagnostic purposes. "
            "Recommendation: Consider creating a 'Diagnostic Mode' exception in the Privacy Scrubber for temporary test IPs or localhost ranges."
        )
    
    if any(word in rationale_lower for word in ['critical', 'urgent', 'emergency', 'production']):
        recommendations.append(
            "Strategic Alignment: Override was justified by operational urgency. "
            "Recommendation: Implement a 'Critical Operations Override' protocol that temporarily relaxes compliance filters with audit logging."
        )
    
    if any(word in rationale_lower for word in ['legacy', 'compatibility', 'deprecated']):
        recommendations.append(
            "Strategic Alignment: Override was needed for backward compatibility. "
            "Recommendation: Create a 'Legacy Agent Registry' that allows specific deprecated agents to bypass certain compliance checks."
        )
    
    avg_score = sum(c.get('compliance_score', 0) for c in disapproving_nodes) / len(disapproving_nodes)
    
    if avg_score < 50:
        recommendations.append(
            f"Mesh Consensus: {len(disapproving_nodes)} nodes rejected with average compliance score of {avg_score:.1f}%. "
            "This suggests a fundamental misalignment between the agent's behavior and mesh-wide compliance standards. "
            "Recommendation: Review and update the agent's compliance filters or consider agent redesign."
        )
    
    if not recommendations:
        return (
            f"Strategic Analysis: {len(disapproving_nodes)} node(s) rejected this agent with an average compliance score. "
            "The human rationale suggests operational necessity, but the mesh consensus indicates compliance concerns. "
            "Recommendation: Review the specific compliance filters that triggered rejection and consider creating targeted exceptions."
        )
    
    return " ".join(recommendations)


def analyze_governance_report(report_data: Dict) -> Dict[str, str]:
    """
    Analyze all entries in a governance report and return strategic recommendations.
    
    Returns: Dict mapping entry index to recommendation string
    """
    recommendations = {}
    
    for idx, entry in enumerate(report_data.get('entries', [])):
        rationale = entry.get('rationale', '')
        conflict_profile = entry.get('conflict_profile', [])
        agent_id = entry.get('agent_id', 'unknown')
        commit_hash = entry.get('commit_hash', 'unknown')
        
        recommendation = analyze_with_llm(
            rationale,
            conflict_profile,
            agent_id,
            commit_hash
        )
        
        recommendations[str(idx)] = recommendation
    
    return recommendations


def main():
    """Main entry point - reads JSON from stdin, outputs recommendations to stdout."""
    if len(sys.argv) > 1 and sys.argv[1] == '--help':
        print("Phoenix Strategic Intent Analyzer")
        print("Usage: python phoenix_analyzer.py < report.json")
        print("Or: python phoenix_analyzer.py <path_to_report.json>")
        sys.exit(0)
    
    # Read input
    if len(sys.argv) > 1:
        # Read from file
        with open(sys.argv[1], 'r', encoding='utf-8') as f:
            report_data = json.load(f)
    else:
        # Read from stdin
        report_data = json.load(sys.stdin)
    
    # Analyze report
    recommendations = analyze_governance_report(report_data)
    
    # Output as JSON
    output = {
        'analyzed_at': datetime.utcnow().isoformat() + 'Z',
        'recommendations': recommendations,
        'llm_available': HAS_OPENAI or HAS_ANTHROPIC or HAS_GEMINI
    }
    
    print(json.dumps(output, indent=2))


if __name__ == '__main__':
    main()
