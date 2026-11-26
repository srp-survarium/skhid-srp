"""Build the two camouflage icon sheets through the porter's atlas_icons_add operations."""
from PIL import Image, ImageDraw


MOVIE = 'flash_movies/lobby_menu.swf'
SMALL = 'LobbyMenu_CamoIcons'
BIG = 'LobbyMenu_CamoIconsBig'


def icon_cell(index):
    """One-based (row, column) of the item FIRST_ITEM + index."""
    # The two shipped sheets have 15 rows per column. Flash uses this stride
    # in both inventory and equipment previews; the porter appends columns.
    column, row = divmod(index, 15)
    return row + 1, column + 8


def icon_operations(pack_dir, images):
    """Write the 72px inventory discs and 140px preview squares for (index, image path) pairs."""
    operations = []
    for symbol in (SMALL, BIG):
        (pack_dir / 'icons' / symbol).mkdir(parents=True, exist_ok=True)
        operations.append({'op': 'atlas_icons_add', 'swf': MOVIE, 'symbol': symbol, 'icons': []})

    # Stock inventory swatches have a circular silhouette. Supersample its
    # edge so the icon has partial coverage when drawn over a shop slot.
    circle = Image.new('L', (288, 288))
    ImageDraw.Draw(circle).ellipse((0, 0, 287, 287), fill=255)
    circle = circle.resize((72, 72), Image.Resampling.LANCZOS)
    for index, path in images:
        row, col = icon_cell(index)
        with Image.open(path) as image:
            rgb = image.convert('RGB')
        for op, size in zip(operations, (72, 140)):
            icon = rgb.resize((size, size), Image.Resampling.LANCZOS)
            if op['symbol'] == SMALL:
                icon.putalpha(circle)
            source = f"icons/{op['symbol']}/r{row:02}c{col:02}.png"
            icon.save(pack_dir / source)
            op['icons'].append({'row': row, 'col': col, 'source': source})
    return operations
