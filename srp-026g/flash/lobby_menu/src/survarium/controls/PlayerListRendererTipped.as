package survarium.controls
{
   import survarium.data.LocalizationData;
   
   public class PlayerListRendererTipped extends survarium.controls.PlayerListRenderer
   {
      
      public function PlayerListRendererTipped()
      {
         super();
         tooltipRendererName = "SrvProfileTipRendererHint";
      }
      
      override public function updateData() : void
      {
         super.updateData();
         if(_data)
         {
            _data.tipText = LocalizationData.getName("show_player_info_hint");
         }
      }
   }
}

