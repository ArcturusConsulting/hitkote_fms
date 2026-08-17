import os

# Define the mappings to replace
REPLACEMENTS = {
    'HITKOTE': 'HITKOTE',
    'hitkote': 'hitkote',
    'Hitkote': 'Hitkote',
}

# Directories to ignore so we don't mess up build artifacts or git history
IGNORE_DIRS = {'.git', 'build', 'install', 'log', '__pycache__'}

def refactor_repo():
    print("Starting refactor: hitkote -> hitkote...")

    # Walk bottom-up (topdown=False) so renaming child directories/files happens before parents
    for root, dirs, files in os.walk('.', topdown=False):
        # Exclude ignored directories
        dirs[:] = [d for d in dirs if d not in IGNORE_DIRS]
        if any(ign in root.split(os.sep) for ign in IGNORE_DIRS):
            continue

        # 1. Replace text content inside files
        for file in files:
            file_path = os.path.join(root, file)
            try:
                with open(file_path, 'r', encoding='utf-8') as f:
                    content = f.read()
                
                new_content = content
                for old, new in REPLACEMENTS.items():
                    new_content = new_content.replace(old, new)
                
                if new_content != content:
                    with open(file_path, 'w', encoding='utf-8') as f:
                        f.write(new_content)
                    print(f"Updated text in: {file_path}")
            except (UnicodeDecodeError, PermissionError):
                # Skip binary files, images, or locked files
                pass

        # 2. Rename files and directories containing the keywords
        for name in files + dirs:
            new_name = name
            for old, new in REPLACEMENTS.items():
                new_name = new_name.replace(old, new)
            
            if new_name != name:
                old_path = os.path.join(root, name)
                new_path = os.path.join(root, new_name)
                os.rename(old_path, new_path)
                print(f"Renamed: {old_path} -> {new_path}")

    print("Refactoring complete!")

if __name__ == '__main__':
    refactor_repo()