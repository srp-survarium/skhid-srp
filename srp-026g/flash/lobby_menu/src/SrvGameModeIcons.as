package
{
   import flash.display.Bitmap;
   import flash.display.BitmapData;
   import flash.display.MovieClip;

   // A match's mode icon on the loading screen (LobbyGameLoad.setMode: frame `ui.icon` + 1). The
   // movie's own three frames (1 the protective device, 2 a transmitter mast, 3 Team Deathmatch's
   // skull), then srp's: 4 Artifact Hunt (`ui.icon` 3, GameModeIconArtefactHunt) and 5 Freeze Tag
   // (`ui.icon` 4, GameModeIconFreezeTag), where the protective device's 61 x 55 lies, in its place
   // (frame 1's art hidden under it).
   public dynamic class SrvGameModeIcons extends MovieClip
   {
      public static const ARTEFACT_HUNT:int = 4;

      public static const FREEZE_TAG:int = 5;

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
            default:
               return null;
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
