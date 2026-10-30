package
{
   import flash.display.BitmapData;

   // The lobby window's card of the Bunker (exe patch 81, docs/notes/lobby-scenes.md): the lobby's
   // stock view, a crop of a game capture (bitmaps/, which the build adds).
   public dynamic class SrpLobbyBunker extends BitmapData
   {

      public function SrpLobbyBunker(w:int = 256, h:int = 144)
      {
         super(w,h);
      }
   }
}
