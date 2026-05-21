#!/usr/bin/env python3
import sys

files = sys.argv[1:]
for f in files:
    with open(f) as fh:
        lines = fh.readlines()
    
    in_test = False
    brace_depth = 0
    found_mod = False
    non_test = []
    
    for i, line in enumerate(lines, 1):
        s = line.strip()
        
        if s == "#[cfg(test)]":
            in_test = True
            found_mod = False
            brace_depth = 0
            continue
        
        if in_test and not found_mod:
            if "mod " in s and "{" in s:
                found_mod = True
                brace_depth = s.count("{") - s.count("}")
                if brace_depth <= 0:
                    in_test = False
            elif "{" in s:
                found_mod = True
                brace_depth = s.count("{") - s.count("}")
            continue
        
        if in_test and found_mod:
            brace_depth += line.count("{") - line.count("}")
            if brace_depth <= 0:
                in_test = False
                found_mod = False
            continue
        
        if ".unwrap()" in s:
            non_test.append((i, s[:120]))
    
    if non_test:
        print(f"\n=== {f} ({len(non_test)} non-test unwraps) ===")
        for ln, text in non_test[:8]:
            print(f"  L{ln}: {text}")
        if len(non_test) > 8:
            print(f"  ... and {len(non_test) - 8} more")
    else:
        print(f"  {f}: 0 non-test unwraps")
