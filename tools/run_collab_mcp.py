#!/Users/rlupi/src/deep-cuts/tools/.venv/bin/python
import os
import sys

# Get the tools directory path
tools_dir = os.path.dirname(os.path.abspath(__file__))

# Add tools_dir to sys.path so we can import the collab_mcp package
if tools_dir not in sys.path:
    sys.path.insert(0, tools_dir)

# Import the main function from the collab_mcp package
from collab_mcp.server import main

if __name__ == '__main__':
    main()
