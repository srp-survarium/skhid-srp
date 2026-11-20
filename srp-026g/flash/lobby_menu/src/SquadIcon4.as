package
{
   import flash.display.BitmapData;

   // The match's squad icon 4 (the letter D in a ring): frame 4 of the hud's squad_icon
   // (PlayerListRenderer's, sprite 518 of the hud movie, the Tab list's squad mark), exported
   // with FFDec at 64 x 64 (bitmaps/, which the build adds). Tinted where it is drawn, as the
   // hud tints it (survarium.rooms.RoomsLook.squadIcon).
   public dynamic class SquadIcon4 extends BitmapData
   {
      public function SquadIcon4(w:int = 64, h:int = 64)
      {
         super(w,h);
      }
   }
}
