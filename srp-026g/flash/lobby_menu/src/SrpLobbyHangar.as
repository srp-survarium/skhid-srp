package
{
   import flash.display.BitmapData;

   // The lobby window's card of the Hangar (exe patch 81, docs/notes/lobby-scenes.md): the lobby's
   // stock view, a crop of a game capture (bitmaps/, which the build adds).
   public dynamic class SrpLobbyHangar extends BitmapData
   {

      public function SrpLobbyHangar(w:int = 256, h:int = 144)
      {
         super(w,h);
      }
   }
}
