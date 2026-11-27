package
{
    import flash.display.Bitmap;
    import flash.display.BitmapData;
    import flash.display.Shape;
    import flash.display.Sprite;
    import flash.utils.Dictionary;

    // The Weapon view window's preview (exe patch 218, docs/notes/weapon-view-settings.md): the AK-12 aimed
    // over its iron sights, an EOTech 552 or a 1P69 Hyperon, in Vostok Radar Station where a player stands,
    // a soldier 30 m ahead down a lane, as the window's values would draw it. Offline renders
    // (scripts/weapon-view-preview): the held gun's layers, each at a few angles (its tiers: the widest its
    // sliders reach, so the whole gun is there at every setting, and narrower ones, sharp when zoomed in),
    // for the iron and red dot views at -20, 0, +25 and +50 cm of sight distance; the world a capture of
    // the map at 100 degrees vertical (the widest world drawn: the iron sights' at uro_fov 100 is 94.8;
    // a closer one at 20 for the scope).
    // - A held angle `a` is the narrowest tier `t` at or over it, scaled about the centre by
    //   tan(t / 2) / tan(a / 2): exact for the angle, as the game's lens is the same scale of everything
    //   held.
    // - The world is the capture scaled the same way to the world's angle at the sight's zoom (patch 22's
    //   65 z + (uro_fov - 65) max(0, (z - 0.25) / 0.75); the iron sights' zoom 0.909, the EOTech's 0.8, the
    //   1P69's 0.25), the player's World FOV.
    // - "Weapon at the world FOV" draws the gun at that world angle, as the game does.
    // - The sight distance crossfades the two renders around it: approximate.
    // The pictures are smoothed Bitmaps scaled on the GPU under a mask (drawn at the screen's own pixels at
    // any UI scale; a BitmapData drawn at the pane's size was enlarged without filtering by the game's
    // Scaleform at 1440p), made only while shown (a 1600x900 layer is 5.8 MB decoded, the world 11.4 MB).
    public class SrpWeaponPreview extends Sprite
    {
        public static const IRON:int = 0;
        public static const RED_DOT:int = 1;
        public static const SCOPE:int = 2;
        public static const WIDTH:Number = 800;
        public static const HEIGHT:Number = 450;
        private static const WORLD_FOV:Number = 100;
        private static const SCOPE_WORLD_FOV:Number = 20;
        // Each view's sight zoom (its fov_factor), and the AK-12's aimed factor over a rifle's (both 0.5).
        private static const ZOOM:Array = [0.909, 0.8, 0.25];
        private static const DISTANCES:Array = [-20, 0, 25, 50];
        // Each view's tiers' angles, widest first (layers.py's TIERS: the widest its sliders reach, then each
        // at most 1.47 times the next's half-angle tangent, the last over the narrowest by as much).
        public static const TIERS:Array = [[94.8, 74.16, 55.42, 40.11, 28.47], [77.7, 57.71, 41.3, 28.91], [32.5, 23.06]];
        // The layers' classes by view, distance and tier (named here so the build keeps them).
        private static const LAYERS:Array = [
            [[SrpPreviewIronM20A, SrpPreviewIronM20B, SrpPreviewIronM20C, SrpPreviewIronM20D, SrpPreviewIronM20E],
             [SrpPreviewIron0A, SrpPreviewIron0B, SrpPreviewIron0C, SrpPreviewIron0D, SrpPreviewIron0E],
             [SrpPreviewIron25A, SrpPreviewIron25B, SrpPreviewIron25C, SrpPreviewIron25D, SrpPreviewIron25E],
             [SrpPreviewIron50A, SrpPreviewIron50B, SrpPreviewIron50C, SrpPreviewIron50D, SrpPreviewIron50E]],
            [[SrpPreviewRedDotM20A, SrpPreviewRedDotM20B, SrpPreviewRedDotM20C, SrpPreviewRedDotM20D],
             [SrpPreviewRedDot0A, SrpPreviewRedDot0B, SrpPreviewRedDot0C, SrpPreviewRedDot0D],
             [SrpPreviewRedDot25A, SrpPreviewRedDot25B, SrpPreviewRedDot25C, SrpPreviewRedDot25D],
             [SrpPreviewRedDot50A, SrpPreviewRedDot50B, SrpPreviewRedDot50C, SrpPreviewRedDot50D]],
            [[SrpPreviewScopeA, SrpPreviewScopeB]]];
        private static const EDGE:uint = 0x585A5D;

        private var pane:Sprite = new Sprite();
        // The world shown (the capture's, or the scope's closer one), made while shown.
        private var world:Bitmap;
        private var worldClass:Class;
        // The gun's layers made, by class.
        private var made:Dictionary = new Dictionary();
        private var view:int = IRON;
        // The pane's edge: its bounds are the preview's (the pictures past it are masked).
        public var frame:Shape = new Shape();
        // What the last draw drew, for the checks: {held, made (the tier's angle), scale (of the picture
        // filling the pane), alpha} each, the world first.
        public var drawn:Array = [];

        public function SrpWeaponPreview()
        {
            name = "_srp_weapon_preview";
            addChild(pane);
            var clip:Shape = new Shape();
            clip.graphics.beginFill(0);
            clip.graphics.drawRect(0, 0, WIDTH, HEIGHT);
            clip.graphics.endFill();
            addChild(clip);
            pane.mask = clip;
            frame.graphics.lineStyle(1, EDGE);
            frame.graphics.drawRect(0, 0, WIDTH - 1, HEIGHT - 1);
            addChild(frame);
        }

        public function get shown():int
        {
            return view;
        }

        public function set shown(value:int):void
        {
            view = value;
        }

        // The world's angle of the view at `uroFov` (patch 22's formula).
        public static function worldAngle(uroFov:Number, zoom:Number):Number
        {
            return 65 * zoom + (uroFov - 65) * Math.max(0, (zoom - 0.25) / 0.75);
        }

        // How big a picture made at `made` degrees is drawn at `angle`.
        public static function scaleOf(made:Number, angle:Number):Number
        {
            return Math.tan(made * Math.PI / 360) / Math.tan(angle * Math.PI / 360);
        }

        // The narrowest of the view's tiers at or over `angle` (the widest when none is).
        public static function tierOf(view:int, angle:Number):int
        {
            var tiers:Array = TIERS[view];
            var at:int = 0;
            while (at + 1 < tiers.length && tiers[at + 1] >= angle - 1e-6)
            {
                at++;
            }
            return at;
        }

        // The preview of the window's values: the sights' and the scopes' degrees (a rifle's), the checkbox,
        // the sight distance in cm, the player's World FOV.
        public function draw(sights:Number, scopes:Number, atWorld:Boolean, distance:Number, uroFov:Number):void
        {
            // before the exe's answers the sliders hold what their range was: the defaults (the exe's ranges:
            // patch 218's, patch 22's uro_fov 50 to 100)
            sights = within(sights, 20, 65, 32.5);
            scopes = within(scopes, 16.25, 32.5, 32.5);
            uroFov = within(uroFov, 50, 100, 65);
            var worldAt:Number = worldAngle(uroFov, ZOOM[view]);
            var held:Number = atWorld ? worldAt : (view == SCOPE ? scopes : sights);
            var scoped:Boolean = view == SCOPE;
            drawn = [];
            var Picture:Class = scoped ? SrpPreviewWorldScope : SrpPreviewWorld;
            if (worldClass != Picture)
            {
                if (world != null)
                {
                    pane.removeChild(world);
                    world.bitmapData.dispose();
                }
                world = bitmap(new Picture());
                worldClass = Picture;
                pane.addChildAt(world, 0);
            }
            place(world, scaleOf(scoped ? SCOPE_WORLD_FOV : WORLD_FOV, worldAt));
            drawn.push({held: worldAt, made: scoped ? SCOPE_WORLD_FOV : WORLD_FOV,
                scale: scaleOf(scoped ? SCOPE_WORLD_FOV : WORLD_FOV, worldAt), alpha: 1});
            var tier:int = tierOf(view, held);
            var made:Number = TIERS[view][tier];
            var rows:Array = LAYERS[view];
            var wanted:Array = [];
            if (rows.length == 1)
            {
                wanted.push([rows[0][tier], 1]);
            }
            else
            {
                // the render at or below the distance, the next one over it by how far the distance is
                // towards it
                var d:Number = Math.max(DISTANCES[0], Math.min(DISTANCES[DISTANCES.length - 1], distance));
                var i:int = 0;
                while (i + 2 < DISTANCES.length && d >= DISTANCES[i + 1])
                {
                    i++;
                }
                var t:Number = (d - DISTANCES[i]) / (DISTANCES[i + 1] - DISTANCES[i]);
                if (t >= 1)
                {
                    wanted.push([rows[i + 1][tier], 1]);
                }
                else
                {
                    wanted.push([rows[i][tier], 1]);
                    if (t > 0)
                    {
                        wanted.push([rows[i + 1][tier], t]);
                    }
                }
            }
            show(wanted, made, held);
        }

        // The layers `wanted` ([class, alpha], bottom first) shown, every other one let go.
        private function show(wanted:Array, made:Number, held:Number):void
        {
            var keep:Dictionary = new Dictionary();
            for each (var w:Array in wanted)
            {
                keep[w[0]] = true;
            }
            for each (var key:Object in made_())
            {
                if (!keep[key])
                {
                    var old:Bitmap = this.made[key];
                    pane.removeChild(old);
                    old.bitmapData.dispose();
                    delete this.made[key];
                }
            }
            var scale:Number = scaleOf(made, held);
            for each (w in wanted)
            {
                var b:Bitmap = this.made[w[0]];
                if (b == null)
                {
                    var Picture:Class = w[0];
                    b = bitmap(new Picture());
                    this.made[w[0]] = b;
                }
                pane.addChild(b);
                place(b, scale);
                b.alpha = w[1];
                drawn.push({held: held, made: made, scale: scale, alpha: w[1]});
            }
        }

        // The layers made (a copy of the keys, so they can be let go while walked).
        private function made_():Array
        {
            var keys:Array = [];
            for (var key:Object in made)
            {
                keys.push(key);
            }
            return keys;
        }

        private static function within(value:Number, least:Number, most:Number, otherwise:Number):Number
        {
            return value >= least && value <= most ? value : otherwise;
        }

        // A picture filling the pane at `scale`, about its centre.
        private static function place(b:Bitmap, scale:Number):void
        {
            var fit:Number = HEIGHT / b.bitmapData.height * scale;
            b.scaleX = b.scaleY = fit;
            b.x = (WIDTH - b.bitmapData.width * fit) / 2;
            b.y = (HEIGHT - b.bitmapData.height * fit) / 2;
        }

        private static function bitmap(data:BitmapData):Bitmap
        {
            var b:Bitmap = new Bitmap(data, "never", true);
            b.smoothing = true;
            return b;
        }
    }
}
