#!/usr/bin/env python3
"""
Ferrellgas-Specific Privacy Scrubber - "Shield" for Internal Infrastructure

This script redacts Ferrellgas-specific hostnames, domains, and naming conventions
before committing to the public pagi-agent-repo. It specifically targets:
- Internal domains (ferrellgas.com, ferrellgas.net, ferrellgas.local)
- Subdomain chains (.internal)
- Machine IDs (FG-SRV-*, SRV-*, WS-* patterns)

Usage:
    python ferrellgas_scrubber.py <filepath> [--dry-run]
"""

import re
import sys
import argparse
import json
from pathlib import Path
from typing import List, Tuple


class FerrellgasScrubber:
    """Privacy filter for Ferrellgas-specific patterns"""
    
    def __init__(self):
        # Internal domains: matches ferrellgas.com, ferrellgas.net, ferrellgas.local
        self.domain_pattern = re.compile(
            r'(?i)[a-z0-9.-]+\.ferrellgas\.(com|net|local)',
            re.IGNORECASE
        )
        
        # Subdomain chains: matches *.internal
        self.internal_pattern = re.compile(
            r'(?i)\b[a-z0-9]+(-[a-z0-9]+)*\.internal\b',
            re.IGNORECASE
        )
        
        # Machine IDs: matches FG-SRV-####, SRV-####, WS-#### patterns
        self.machine_id_pattern = re.compile(
            r'\b(FG|SRV|WS)-[0-9]{4,}\b',
            re.IGNORECASE
        )
        
        # Combined pattern for finding all matches
        self.all_patterns = [
            (self.domain_pattern, "domain"),
            (self.internal_pattern, "internal"),
            (self.machine_id_pattern, "machine_id"),
        ]
    
    def scrub_content(self, content: str) -> Tuple[str, List[Tuple[str, str]]]:
        """
        Scrub sensitive information from content.
        
        Returns:
            Tuple of (scrubbed_content, list of (pattern_type, matched_text))
        """
        scrubbed = content
        matches_found = []
        
        for pattern, pattern_type in self.all_patterns:
            for match in pattern.finditer(content):
                matched_text = match.group(0)
                matches_found.append((pattern_type, matched_text))
                scrubbed = scrubbed.replace(matched_text, "[Phoenix-Redacted]")
        
        return scrubbed, matches_found
    
    def scrub_file(self, filepath: Path, dry_run: bool = False, json_output: bool = False) -> List[Tuple[str, str]]:
        """
        Scrub a file, replacing sensitive patterns.
        
        Args:
            filepath: Path to the file to scrub
            dry_run: If True, only report what would be redacted without modifying the file
            json_output: If True, output JSON with clean_text and redaction_count instead of modifying file
            
        Returns:
            List of (pattern_type, matched_text) tuples
        """
        if not filepath.exists():
            if json_output:
                print(json.dumps({"error": f"File not found: {filepath}"}), file=sys.stderr)
            else:
                print(f"Error: File not found: {filepath}", file=sys.stderr)
            return []
        
        try:
            with open(filepath, 'r', encoding='utf-8') as f:
                content = f.read()
        except Exception as e:
            if json_output:
                print(json.dumps({"error": f"Error reading file: {e}"}), file=sys.stderr)
            else:
                print(f"Error reading file {filepath}: {e}", file=sys.stderr)
            return []
        
        scrubbed_content, matches = self.scrub_content(content)
        
        if json_output:
            # Output JSON format for programmatic use
            result = {
                "clean_text": scrubbed_content,
                "redaction_count": len(matches)
            }
            print(json.dumps(result))
            return matches
        
        if matches:
            if dry_run:
                print(f"\n[DRY RUN] Would redact {len(matches)} pattern(s) in {filepath}:")
                for pattern_type, matched_text in matches:
                    print(f"  - {pattern_type}: {matched_text}")
            else:
                try:
                    with open(filepath, 'w', encoding='utf-8') as f:
                        f.write(scrubbed_content)
                    print(f"✓ Scrubbed {len(matches)} pattern(s) in {filepath}")
                    for pattern_type, matched_text in matches:
                        print(f"  - {pattern_type}: {matched_text}")
                except Exception as e:
                    print(f"Error writing file {filepath}: {e}", file=sys.stderr)
                    return []
        else:
            if dry_run:
                print(f"[DRY RUN] No patterns found in {filepath}")
            else:
                print(f"✓ No patterns to scrub in {filepath}")
        
        return matches


def main():
    parser = argparse.ArgumentParser(
        description="Scrub Ferrellgas-specific sensitive information from files"
    )
    parser.add_argument(
        "filepath",
        type=str,
        help="Path to the file to scrub"
    )
    parser.add_argument(
        "--dry-run",
        action="store_true",
        help="Print what would be redacted without modifying the file"
    )
    parser.add_argument(
        "--json",
        action="store_true",
        help="Output JSON format with clean_text and redaction_count (for programmatic use)"
    )
    
    args = parser.parse_args()
    
    filepath = Path(args.filepath)
    scrubber = FerrellgasScrubber()
    
    matches = scrubber.scrub_file(filepath, dry_run=args.dry_run, json_output=args.json)
    
    if matches:
        sys.exit(0)
    else:
        sys.exit(0)


if __name__ == "__main__":
    main()
