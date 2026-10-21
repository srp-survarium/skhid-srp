package survarium.controls
{
   import flash.display.MovieClip;
   import scaleform.clik.controls.Button;
   import scaleform.clik.controls.ListItemRenderer;
   
   public class ClanTeamListRenderer extends ListItemRenderer
   {
      
      public var playerName:FixedTextDots;
      
      public var favoritedIcon:MovieClip;
      
      public var btnAddToFavorite:Button;
      
      public var btnMovePlayer:Button;
      
      public var btnMovePlayerUp:Button;
      
      public var separator:MovieClip;
      
      public var iconReady:MovieClip;
      
      public function ClanTeamListRenderer()
      {
         super();
      }
      
      override public function setData(newData:Object) : void
      {
         _data = newData;
         invalidateData();
      }
      
      override protected function updateText() : void
      {
         var isHovered:Boolean;
         if(_data)
         {
            mouseChildren = true;
            this.separator.visible = _data.status == -1;
            this.favoritedIcon.visible = _data.status == 0 && !this.separator.visible;
            this.playerName.visible = !this.separator.visible;
            isHovered = _state == "over" || _state == "down" || _selected;
            this.btnAddToFavorite.visible = !this.favoritedIcon.visible && !this.separator.visible && isHovered && _data.favoritable;
            this.btnMovePlayer.visible = !this.favoritedIcon.visible && !this.separator.visible && isHovered && _data.movableDown;
            this.btnMovePlayerUp.visible = !this.favoritedIcon.visible && !this.separator.visible && isHovered && _data.movableUp;
            this.iconReady.visible = !this.favoritedIcon.visible && !this.separator.visible && (!isHovered || this.btnMovePlayerUp.visible == false && this.btnMovePlayer.visible == false) && _data.ready == true;
            visible = true;
            this.separator.textField.text = _data.status_text;
            this.playerName.text = _data.name;
         }
         else
         {
            visible = false;
         }
      }
      
      override public function toString() : String
      {
         return "[CLIK ClanTeamListRenderer " + index + ", " + name + "]";
      }
      
      override protected function configUI() : void
      {
         super.configUI();
         preventAutosizing = true;
      }
   }
}

