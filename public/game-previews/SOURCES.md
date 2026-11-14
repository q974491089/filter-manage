# Built-in game preview images

These are publisher-provided marketing screenshots used as fixed color-test
originals for the built-in effect preview. Each image is credited with the game,
publisher and the store page it came from in `sources.json`.

Copyright of these screenshots belongs to their respective publishers
(Team Jade / TiMi Studio Group, Valve, Riot Games). They are redistributed here
unmodified apart from resizing to 1280 x 720 and PNG conversion, solely so the app
can render an offline color-simulation preview. Image rights apply to these assets
and their color-adjusted derivatives; they do not change the license of the
surrounding application. If a publisher asks for removal, drop the matching file
in `sources/` and rebuild the derived PNGs.

Scene meanings:

| id | label | game | purpose |
|----|-------|------|---------|
| `deltaforce` | 室内外曝光 | Delta Force / 三角洲行动 | 暗室内景 + 拱窗高光，观察暗部细节与高光溢出 |
| `cs2` | 户外强光 | Counter-Strike 2 | 白天户外大面积石墙与阴影，观察对比度 / 色温 |
| `valorant` | 霓虹色彩 | VALORANT / 瓦洛兰特 | 高饱和霓虹与技能特效，观察饱和度与色彩偏差 |

Exact original files are in `sources/`; source URLs, author credit and checksums
are in `sources.json`. Derived PNGs are resized to 1280 x 720 and converted to RGB
without applying any game filter. Preview outputs apply only ICC VCGT and the
selected global color parameters. Original source files remain available
unchanged.

Sources: <https://store.steampowered.com/app/2507950/>,
<https://store.steampowered.com/app/730/>,
<https://store.epicgames.com/p/valorant>
