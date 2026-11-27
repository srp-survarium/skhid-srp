package
{
    import flash.display.Bitmap;
    import flash.display.BitmapData;
    import flash.display.Shape;
    import flash.display.Sprite;
    import flash.geom.ColorTransform;
    import flash.geom.Matrix;

    // The Weapon view window's preview (exe patch 218, docs/notes/weapon-view-settings.md): the AK-12 aimed
    // over its iron sights, an EOTech 552 or a 1P69 Hyperon, over Vostok Radar Station with a soldier
    // standing 30 m ahead in a yard, as the window's values would draw it. Offline renders
    // (scripts/weapon-view-preview): the held gun's layers at 0.69d0's 32.5 degrees (and the iron and red
    // dot views at -20, 0, +25 and +50 cm of sight distance), the world a capture of the map at 100 degrees
    // vertical (the widest world drawn: the iron sights' at uro_fov 100 is 94.8; a closer one at 20 for the
    // scope).
    // - A held angle is the layer scaled about the centre by tan(16.25) / tan(angle / 2): exact for the
    //   angle, as the game's lens is the same scale of everything held.
    // - The world is the capture scaled the same way to the world's angle at the sight's zoom (patch 22's
    //   65 z + (uro_fov - 65) max(0, (z - 0.25) / 0.75); the iron sights' zoom 0.909, the EOTech's 0.8, the
    //   1P69's 0.25), the player's World FOV.
    // - "Weapon at the world FOV" draws the gun at that world angle, as the game does.
    // - The sight distance crossfades the two renders around it: approximate (the note under it says so).
    public class SrpWeaponPreview extends Sprite
    {
        public static const IRON:int = 0;
        public static const RED_DOT:int = 1;
        public static const SCOPE:int = 2;
        public static const WIDTH:Number = 300;
        public static const HEIGHT:Number = 169;
        // The renders' angle (0.69d0's aimed rifle) and the captures'.
        private static const RENDER_FOV:Number = 32.5;
        private static const WORLD_FOV:Number = 100;
        private static const SCOPE_WORLD_FOV:Number = 20;
        // Each view's sight zoom (its fov_factor), and the AK-12's aimed factor over a rifle's (both 0.5).
        private static const ZOOM:Array = [0.909, 0.8, 0.25];
        private static const DISTANCES:Array = [-20, 0, 25, 50];
        private static const EDGE:uint = 0x585A5D;

        // The pictures (BitmapData) by name: the worlds, and the gun's layers by view (the iron and red dot
        // ones by distance, the scope's one).
        private var world:BitmapData = new SrpPreviewWorld();
        private var scopeWorld:BitmapData = new SrpPreviewWorldScope();
        private var layers:Array = [[new SrpPreviewIronM20(), new SrpPreviewIron0(), new SrpPreviewIron25(), new SrpPreviewIron50()],
            [new SrpPreviewRedDotM20(), new SrpPreviewRedDot0(), new SrpPreviewRedDot25(), new SrpPreviewRedDot50()],
            [new SrpPreviewScope()]];
        // The pane: the pictures drawn into one bitmap of its size, so nothing reaches past it.
        private var canvas:BitmapData = new BitmapData(WIDTH, HEIGHT, false, 0);
        private var view:int = IRON;
        // What the last draw drew, for the checks: {scale (of the picture filling the pane), alpha} each,
        // the world first.
        public var drawn:Array = [];

        public function SrpWeaponPreview()
        {
            name = "_srp_weapon_preview";
            var pane:Bitmap = new Bitmap(canvas);
            pane.smoothing = true;
            addChild(pane);
            var frame:Shape = new Shape();
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

        // The preview of the window's values: the sights' and the scopes' degrees (a rifle's), the checkbox,
        // the sight distance in cm, the player's World FOV.
        public function draw(sights:Number, scopes:Number, atWorld:Boolean, distance:Number, uroFov:Number):void
        {
            // before the exe's answers the sliders hold what their range was: the defaults (the exe's ranges:
            // patch 218's, patch 22's uro_fov 50 to 100)
            sights = within(sights, 20, 65, RENDER_FOV);
            scopes = within(scopes, 16.25, 32.5, RENDER_FOV);
            uroFov = within(uroFov, 50, 100, 65);
            var worldAt:Number = worldAngle(uroFov, ZOOM[view]);
            var held:Number = atWorld ? worldAt : (view == SCOPE ? scopes : sights);
            var scoped:Boolean = view == SCOPE;
            canvas.fillRect(canvas.rect, 0);
            drawn = [];
            paint(scoped ? scopeWorld : world, scaleOf(scoped ? SCOPE_WORLD_FOV : WORLD_FOV, worldAt), 1);
            var row:Array = layers[view];
            var scale:Number = scaleOf(RENDER_FOV, held);
            if (row.length == 1)
            {
                paint(row[0], scale, 1);
                return;
            }
            // the render at or below the distance, the next one over it by how far the distance is towards it
            var d:Number = Math.max(DISTANCES[0], Math.min(DISTANCES[DISTANCES.length - 1], distance));
            var i:int = 0;
            while (i + 2 < DISTANCES.length && d >= DISTANCES[i + 1])
            {
                i++;
            }
            var t:Number = (d - DISTANCES[i]) / (DISTANCES[i + 1] - DISTANCES[i]);
            if (t >= 1)
            {
                paint(row[i + 1], scale, 1);
                return;
            }
            paint(row[i], scale, 1);
            if (t > 0)
            {
                paint(row[i + 1], scale, t);
            }
        }

        private static function within(value:Number, least:Number, most:Number, otherwise:Number):Number
        {
            return value >= least && value <= most ? value : otherwise;
        }

        // A picture over the pane: filling it at `scale`, about its centre, at `alpha`.
        private function paint(picture:BitmapData, scale:Number, alpha:Number):void
        {
            var fit:Number = HEIGHT / picture.height * scale;
            var m:Matrix = new Matrix(fit, 0, 0, fit, (WIDTH - picture.width * fit) / 2, (HEIGHT - picture.height * fit) / 2);
            canvas.draw(picture, m, alpha < 1 ? new ColorTransform(1, 1, 1, alpha) : null, null, null, true);
            drawn.push({scale: scale, alpha: alpha});
        }
    }
}
