package survarium.controls
{
   import flash.text.TextField;
   import scaleform.clik.controls.ListItemRenderer;
   import survarium.constants.Colors;
   import survarium.data.LocalizationData;
   
   public class ClanListRenderer extends ListItemRenderer
   {
      
      public var playerName:FixedTextDots;
      
      public var textRank:TextField;
      
      public var textLevel:TextField;
      
      public var textStatus:TextField;
      
      public function ClanListRenderer()
      {
         super();
      }
      
      override public function setData(param1:Object) : void
      {
         _data = param1;
         invalidateData();
      }
      
      override protected function updateText() : void
      {
         var _loc1_:Boolean;
         if(_data)
         {
            visible = true;
            this.playerName.text = _data.name;
            this.playerName.textField.textColor = _selected ? Colors.HIGHLIGHT : Colors.LIGHT_GRAY;
            this.playerName.y = _selected || _state != "down" ? 0 : 1;
            this.textRank.text = _data.default_rank == true ? LocalizationData.getName("clan_rank_" + _data.rank) : _data.rank;
            this.textLevel.text = _data.level;
            this.textStatus.text = _data.status;
            _loc1_ = _data.online == undefined || _data.online == true;
            this.textStatus.textColor = _loc1_ ? Colors.LIGHT_GRAY : Colors.NOT_READY;
            alpha = _loc1_ ? 1 : 0.7;
         }
         else
         {
            visible = false;
         }
      }
      
      override public function toString() : String
      {
         return "[CLIK ClanListItemRenderer " + index + ", " + name + "]";
      }
      
      override protected function configUI() : void
      {
         super.configUI();
         preventAutosizing = true;
      }
   }
}

