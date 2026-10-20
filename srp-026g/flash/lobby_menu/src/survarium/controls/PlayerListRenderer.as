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
      
      override public function set focusable(param1:Boolean) : void
      {
      }
      
      public function get index() : uint
      {
         return this._index;
      }
      
      public function set index(param1:uint) : void
      {
         _index = param1;
      }
      
      public function get selectable() : Boolean
      {
         return this._selectable;
      }
      
      public function set selectable(param1:Boolean) : void
      {
         _selectable = param1 && enabled;
      }
      
      public function setListData(param1:ListData) : void
      {
         index = param1.index;
         selected = param1.selected;
      }
      
      public function setData(param1:Object) : void
      {
         _data = param1;
         invalidateData();
      }
      
      public function updateData() : void
      {
         var _loc1_:ColorTransform;
         var _loc2_:int;
         if(_data)
         {
            this.ui_name.text = _data.name;
            if(_data.online != undefined && _data.online == false)
            {
               this.ui_name.textColor = 10066329;
            }
            else
            {
               _loc2_ = _data.isLocal ? (Colors.PLAYER) : 16777215;
               this.ui_name.textColor = _loc2_;
            }
            this.ui_kills.text = _data.kills;
            this.ui_deaths.text = _data.deaths;
            this.ui_rank.text = _data.rank;
            if(this.ui_score)
            {
               this.ui_score.text = _data.score;
            }
            if(this.background)
            {
               if(_data.isLocal == true)
               {
                  _loc1_ = new ColorTransform();
                  _loc1_.color = 16777215;
                  this.background.transform.colorTransform = _loc1_;
                  this.background.visible = true;
                  this.background.alpha = 0.15;
               }
               else
               {
                  this.background.visible = this.index % 2 == 0;
                  this.background.transform.colorTransform = null;
                  this.background.alpha = 0.25;
               }
            }
            if(this.squad_icon)
            {
               if(_data.squad_state == 1 || _data.squad_state == 2)
               {
                  this.squad_icon.visible = true;
                  this.squad_icon.gotoAndStop(_data.squad_id);
                  if(_data.team != 1)
                  {
                     this.ui_name.textColor = 16777215;
                     _loc1_ = new ColorTransform();
                     _loc1_.color = 16711680;
                     this.squad_icon.transform.colorTransform = _loc1_;
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
                     _loc1_ = new ColorTransform();
                     _loc1_.color = _data.isLocalSquad ? 11790999 : 10321495;
                     this.squad_icon.transform.colorTransform = _loc1_;
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

