#!/usr/bin/env python3
"""
Privacy Validation Script - Automated "Customs Agent"
Scans .md and .yaml files for sensitive bare-metal information:
- IPv4 addresses
- Local file paths (Users/home/root)
- High-entropy strings labeled KEY, TOKEN, or SECRET
"""

import re
import os
import sys
from pathlib import Path
from typing import List, Tuple, Set


# Pattern definitions
IPV4_PATTERN = r'\b(?:\d{1,3}\.){3}\d{1,3}\b'
LOCAL_PATH_PATTERN = r'/(?:Users|home|root)/[a-zA-Z0-9._-]+'
SECRET_PATTERN = r'(?i)(?:KEY|TOKEN|SECRET)\s*[=:]\s*[a-zA-Z0-9+/=]{20,}'


def load_privacy_ignore() -> Set[str]:
    """Load patterns from .privacyignore file if it exists."""
    ignore_patterns = set()
    privacyignore_path = Path('.privacyignore')
    
    if privacyignore_path.exists():
        with open(privacyignore_path, 'r', encoding='utf-8') as f:
            for line in f:
                line = line.strip()
                if line and not line.startswith('#'):
                    ignore_patterns.add(line)
    
    return ignore_patterns


def should_ignore_match(match: str, ignore_patterns: Set[str]) -> bool:
    """Check if a match should be ignored based on .privacyignore patterns."""
    for pattern in ignore_patterns:
        if re.search(pattern, match):
            return True
    return False


def scan_file(file_path: Path, ignore_patterns: Set[str]) -> List[Tuple[str, str, int, str]]:
    """
    Scan a file for sensitive patterns.
    Returns list of (pattern_type, match, line_number, line_content) tuples.
    """
    violations = []
    
    try:
        with open(file_path, 'r', encoding='utf-8', errors='ignore') as f:
            lines = f.readlines()
            
            for line_num, line in enumerate(lines, 1):
                # Check IPv4 addresses
                ipv4_matches = re.findall(IPV4_PATTERN, line)
                for match in ipv4_matches:
                    if not should_ignore_match(match, ignore_patterns):
                        violations.append(('IPv4', match, line_num, line.strip()))
                
                # Check local paths
                path_matches = re.findall(LOCAL_PATH_PATTERN, line)
                for match in path_matches:
                    if not should_ignore_match(match, ignore_patterns):
                        violations.append(('Local Path', match, line_num, line.strip()))
                
                # Check secrets
                secret_matches = re.findall(SECRET_PATTERN, line)
                for match in secret_matches:
                    if not should_ignore_match(match, ignore_patterns):
                        violations.append(('Secret', match, line_num, line.strip()))
    
    except Exception as e:
        print(f"Error reading {file_path}: {e}", file=sys.stderr)
    
    return violations


def find_target_files(root_dir: Path = Path('.')) -> List[Path]:
    """Find all .md and .yaml files in the repository."""
    target_files = []
    
    # Common directories to skip
    skip_dirs = {'.git', '__pycache__', 'node_modules', '.venv', 'venv', 'target', 'dist', 'build'}
    
    for file_path in root_dir.rglob('*.md'):
        if not any(skip in file_path.parts for skip in skip_dirs):
            target_files.append(file_path)
    
    for file_path in root_dir.rglob('*.yaml'):
        if not any(skip in file_path.parts for skip in skip_dirs):
            target_files.append(file_path)
    
    for file_path in root_dir.rglob('*.yml'):
        if not any(skip in file_path.parts for skip in skip_dirs):
            target_files.append(file_path)
    
    return target_files


def main():
    """Main validation function."""
    root_dir = Path('.')
    ignore_patterns = load_privacy_ignore()
    target_files = find_target_files(root_dir)
    
    all_violations = []
    
    for file_path in target_files:
        violations = scan_file(file_path, ignore_patterns)
        if violations:
            all_violations.extend([(file_path, v) for v in violations])
    
    if all_violations:
        print("[FAIL] Privacy validation failed! Sensitive information detected:\n", file=sys.stderr)
        print("=" * 80, file=sys.stderr)
        
        for file_path, (pattern_type, match, line_num, line_content) in all_violations:
            rel_path = file_path.relative_to(root_dir)
            print(f"\n[FILE] {rel_path}", file=sys.stderr)
            print(f"   Type: {pattern_type}", file=sys.stderr)
            print(f"   Match: {match}", file=sys.stderr)
            print(f"   Line {line_num}: {line_content[:100]}", file=sys.stderr)
        
        print("\n" + "=" * 80, file=sys.stderr)
        print(f"\n[ERROR] Found {len(all_violations)} violation(s).", file=sys.stderr)
        print("Please remove sensitive information or add exemptions to .privacyignore", file=sys.stderr)
        sys.exit(1)
    else:
        print("[PASS] Privacy validation passed! No sensitive information detected.")
        sys.exit(0)


if __name__ == '__main__':
    main()
