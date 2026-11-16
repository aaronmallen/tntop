# Bundled btop themes

These 49 `.theme` files come from
[aristocratos/btop](https://github.com/aristocratos/btop/tree/5d03929a45e0720a4c8e2d189360ff1eb9ce1731/themes),
commit `5d03929a45e0720a4c8e2d189360ff1eb9ce1731`.

The theme files retain upstream colors and author comments without changes.
`LICENSE` contains upstream's Apache-2.0 license.

To refresh the collection, choose a new upstream commit, download its source
archive, and replace every `.theme` file with the matching file in `themes/`.
Copy its license and any notices. Update the commit above, compare filenames and
file contents against that archive, then run the theme parser tests and package
checks. Review added and removed themes before committing.

Ordinary builds use these local files and do not download themes.

Release binaries embed every theme. Set `theme = "dracula"` in the config file,
or use another filename without `.theme`. `Default` selects tntop's built-in
colors. No theme-folder installation is needed.

User themes live in `themes/` beside the selected config file. A user theme with
the same name overrides the bundled copy; `Default` and `TTY` stay reserved. Names are
case-sensitive and appear once in the sorted catalog. Explicit paths still work,
including paths relative to the config file, such as `theme = "themes/mine.theme"`.
