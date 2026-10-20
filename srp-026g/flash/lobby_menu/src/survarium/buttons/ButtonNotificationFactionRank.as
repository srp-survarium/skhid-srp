package survarium.buttons
{
   import flash.display.MovieClip;
   
   public class ButtonNotificationFactionRank extends ButtonNotificationLog
   {
      
      public var icon:MovieClip;
      
      public function ButtonNotificationFactionRank()
      {
         super();
      }
      
      override protected function configUI() : void
      {
         super.configUI();
      }
      
      override protected function updateText() : void
      {
         super.updateText();
         if(_data)
         {
            this.icon.gotoAndStop(_data.faction_id);
            background.height = 70;
         }
      }
   }
}

