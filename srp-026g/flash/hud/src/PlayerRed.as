package
{
   import survarium.controls.MinimapObjectPlayer;
   
   public dynamic class PlayerRed extends MinimapObjectPlayer
   {
      
      public function PlayerRed()
      {
         super();
      }

      // An enemy's name on the Tab map: red, the colour the game gives an enemy's name over its head
      // (render_name 0x1F300F, 0xFF0000).
      override protected function get nameColor() : uint
      {
         return 0xFF0000;
      }
   }
}

