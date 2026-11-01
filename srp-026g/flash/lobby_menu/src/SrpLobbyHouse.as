package
{
   import flash.display.BitmapData;

   // The lobby window's card of the House (exe patch 81, docs/notes/lobby-scenes.md): the lobby's
   // stock view without anyone in it, a crop of a game capture at twice the card's size (bitmaps/,
   // a JPEG, which the build adds).
   public dynamic class SrpLobbyHouse extends BitmapData
   {

      public function SrpLobbyHouse(w:int = 560, h:int = 380)
      {
         super(w,h);
      }
   }
}
