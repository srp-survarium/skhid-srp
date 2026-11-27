package
{
   import flash.display.Bitmap;
   import flash.display.BitmapData;
   import flash.display.MovieClip;

   // A match's mode icon on the loading screen (LobbyGameLoad.setMode: frame `ui.icon` + 1). The
   // movie's own three frames (1 the protective device, 2 a transmitter mast, 3 Team Deathmatch's
   // skull), then srp's: 4 Artifact Hunt (`ui.icon` 3, GameModeIconArtefactHunt), 5 Freeze Tag
   // (`ui.icon` 4, GameModeIconFreezeTag), 6 Free for All (`ui.icon` 5), and a free-for-all's
   // twists, which have no config of their own (docs/notes/ffa.md, "Twists"): 7 Racket, 8 Saloon,
   // 9 Saloon (Winchester), 10 Roulette, 11 Pistols (two TT-33s crossed, in Racket's steel), shown
   // for Free for All's icon by the twist the room's match plays (forTwist); each where the
   // protective device's 61 x 55 lies, in its place (frame 1's art hidden under it). The owner
   // picked the first five pictures (2026-11-22).
   public dynamic class SrvGameModeIcons extends MovieClip
   {
      public static const ARTEFACT_HUNT:int = 4;

      public static const FREEZE_TAG:int = 5;

      public static const FREE_FOR_ALL:int = 6;

      public static const RACKET:int = 7;

      public static const SALOON:int = 8;

      public static const SALOON_WINCHESTER:int = 9;

      public static const ROULETTE:int = 10;

      public static const PISTOLS:int = 11;

      // srp's frames' bitmaps, made at their first showing; the one shown, if any.
      private const own:Object = {};

      private var shown:Bitmap;

      public function SrvGameModeIcons()
      {
         super();
      }

      private static function art(frame:int) : BitmapData
      {
         switch(frame)
         {
            case ARTEFACT_HUNT:
               return new GameModeIconArtefactHunt();
            case FREEZE_TAG:
               return new GameModeIconFreezeTag();
            case FREE_FOR_ALL:
               return new GameModeIconFreeForAll();
            case RACKET:
               return new GameModeIconRacket();
            case SALOON:
               return new GameModeIconSaloon();
            case SALOON_WINCHESTER:
               return new GameModeIconSaloonWinchester();
            case ROULETTE:
               return new GameModeIconRoulette();
            case PISTOLS:
               return new GameModeIconPistols();
            default:
               return null;
         }
      }

      // The `ui.icon` the loading screen shows for a config's `icon` while the room's match plays the
      // lobby mode `twist` (its number, RoomsText.MODES; 0 none known): a twist's own for Free for
      // All's, else the config's.
      public static function forTwist(icon:uint, twist:int) : uint
      {
         if(icon + 1 != FREE_FOR_ALL)
         {
            return icon;
         }
         switch(twist)
         {
            case 9:
               return RACKET - 1;
            case 10:
               return SALOON - 1;
            case 11:
               return SALOON_WINCHESTER - 1;
            case 14:
            case 15:
            case 16:
               return ROULETTE - 1;
            case 18:
               return PISTOLS - 1;
            default:
               return icon;
         }
      }

      override public function gotoAndStop(frame:Object, scene:String = null) : void
      {
         var number:int = int(frame);
         var bitmap:Bitmap = this.own[number] as Bitmap;
         if(bitmap == null)
         {
            var data:BitmapData = art(number);
            if(data != null)
            {
               bitmap = new Bitmap(data,"auto",true);
               this.own[number] = bitmap;
            }
         }
         super.gotoAndStop(bitmap != null ? 1 : frame,scene);
         if(this.shown != null && this.shown != bitmap)
         {
            removeChild(this.shown);
         }
         this.shown = bitmap;
         if(bitmap != null && bitmap.parent != this)
         {
            addChild(bitmap);
         }
         for(var i:int = 0; i < numChildren; i++)
         {
            var child:* = getChildAt(i);
            if(child != bitmap)
            {
               child.visible = bitmap == null;
            }
         }
      }
   }
}
