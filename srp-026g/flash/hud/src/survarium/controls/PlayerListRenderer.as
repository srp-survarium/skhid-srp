package survarium.controls
{
   import flash.display.MovieClip;
   import flash.display.Sprite;
   import flash.geom.ColorTransform;
   import flash.text.TextField;
   import scaleform.clik.constants.InvalidationType;
   import scaleform.clik.controls.Button;
   import scaleform.clik.data.ListData;
   import scaleform.clik.interfaces.IListItemRenderer;
   import survarium.constants.Colors;
   import survarium.constants.SquadState;
   import survarium.constants.TeamSide;
   
   public class PlayerListRenderer extends Button implements IListItemRenderer
   {
      
      protected var _index:uint = 0;
      
      protected var _selectable:Boolean = false;
      
      public var ui_name:FixedTextDots;
      
      public var ui_ping:TextField;
      
      public var ui_kills:TextField;
      
      public var ui_deaths:TextField;
      
      public var ui_rank:TextField;
      
      public var ui_score:TextField;
      
      public var ui_artifacts:TextField;
      
      public var background:Sprite;
      
      public var squad_icon:MovieClip;
      
      public var ui_sqad:TextField;
      
      public function PlayerListRenderer()
      {
         super();
      }
      
      override public function get focusable() : Boolean
      {
         return _focusable;
      }
      
      override public function set focusable(value:Boolean) : void
      {
      }
      
      public function get index() : uint
      {
         return this._index;
      }
      
      public function set index(value:uint) : void
      {
         _index = value;
      }
      
      public function get selectable() : Boolean
      {
         return this._selectable;
      }
      
      public function set selectable(value:Boolean) : void
      {
         _selectable = value && enabled;
      }
      
      public function setListData(listData:ListData) : void
      {
         index = listData.index;
         selected = listData.selected;
      }
      
      public function setData(data:Object) : void
      {
         _data = data;
         invalidateData();
      }
      
      public function updateData() : void
      {
         var colorTransform:ColorTransform;
         var clr:int;
         if(_data)
         {
            this.ui_name.text = _data.name;
            if(_data.online != undefined && _data.online == false)
            {
               this.ui_name.textColor = 10066329;
            }
            else
            {
               clr = _data.isLocal ? (Colors.PLAYER) : 16777215;
               this.ui_name.textColor = clr;
            }
            this.ui_kills.text = _data.kills;
            this.ui_deaths.text = _data.deaths;
            this.ui_rank.text = _data.rank;
            if(this.ui_score)
            {
               this.ui_score.text = _data.score;
            }
            this.showStats(!this.namesOnly);
            if(this.background)
            {
               if(_data.isLocal == true)
               {
                  colorTransform = new ColorTransform();
                  colorTransform.color = 16777215;
                  this.background.transform.colorTransform = colorTransform;
                  this.background.visible = true;
                  this.background.alpha = 0.15;
               }
               else
               {
                  this.background.visible = this.index % 2 == 0;
                  // the identity: Flash and Ruffle refuse null (#2007), which Scaleform took
                  this.background.transform.colorTransform = new ColorTransform();
                  this.background.alpha = 0.25;
               }
            }
            if(this.squad_icon)
            {
               if(_data.squad_state == SquadState.IN_SQUAD || _data.squad_state == SquadState.STATE_2)
               {
                  this.squad_icon.visible = true;
                  this.squad_icon.gotoAndStop(_data.squad_id);
                  if(_data.team != TeamSide.ALLY)
                  {
                     this.ui_name.textColor = 16777215;
                     colorTransform = new ColorTransform();
                     colorTransform.color = 16711680;
                     this.squad_icon.transform.colorTransform = colorTransform;
                  }
                  else
                  {
                     if(_data.isLocal)
                     {
                        this.ui_name.textColor = Colors.PLAYER;
                     }
                     else
                     {
                        this.ui_name.textColor = _data.isLocalSquad ? 11790999 : 16777215;
                     }
                     colorTransform = new ColorTransform();
                     colorTransform.color = _data.isLocalSquad ? 11790999 : 10321495;
                     this.squad_icon.transform.colorTransform = colorTransform;
                  }
                  if(_data.online != undefined && _data.online == false)
                  {
                     this.ui_name.textColor = 10066329;
                  }
               }
               else
               {
                  this.squad_icon.visible = false;
                  this.squad_icon.gotoAndStop(1);
               }
            }
            if(_data.color != undefined)
            {
               this.ui_name.textColor = _data.color;
            }
         }
         else
         {
            if(this.ui_score)
            {
               this.ui_score.text = "";
            }
            this.ui_name.text = "";
            this.ui_kills.text = "";
            this.ui_deaths.text = "";
            this.ui_rank.text = "";
            if(this.squad_icon)
            {
               this.squad_icon.visible = false;
               this.squad_icon.gotoAndStop(1);
            }
         }
      }
      
      // The list this row is in shows names only (PlayerList.namesOnly: the Settings' "Hide player scores").
      protected function get namesOnly() : Boolean
      {
         var board:PlayerList = owner != null ? owner.parent as PlayerList : null;
         return board != null && board.namesOnly;
      }
      
      protected function showStats(shown:Boolean) : void
      {
         this.ui_kills.visible = this.ui_deaths.visible = this.ui_rank.visible = shown;
         if(this.ui_score)
         {
            this.ui_score.visible = shown;
         }
      }
      
      override public function toString() : String
      {
         return "[CLIK PlayerListItemRenderer " + this.index + ", " + name + "]";
      }
      
      override protected function draw() : void
      {
         super.draw();
         if(isInvalid(InvalidationType.DATA))
         {
            this.updateData();
         }
      }
      
      override protected function configUI() : void
      {
         super.configUI();
         _focusable = tabEnabled = tabChildren = mouseChildren = false;
         preventAutosizing = true;
         doubleClickEnabled = true;
      }
   }
}

