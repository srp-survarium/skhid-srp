package survarium.controls
{
   import flash.display.MovieClip;
   import flash.display.Sprite;
   import flash.events.Event;
   import flash.events.MouseEvent;
   import scaleform.clik.controls.Button;
   import scaleform.clik.data.ListData;
   import scaleform.clik.interfaces.IListItemRenderer;
   import scaleform.gfx.MouseEventEx;
   
   public class FilterItem extends Button implements IListItemRenderer
   {
      
      protected var _index:uint = 0;
      
      protected var _selectable:Boolean = true;
      
      protected var _content:Button;
      
      public var contentCanvas:Sprite;
      
      public var content:*;
      
      public var spriteAnchor:Sprite = new Sprite();
      
      public var itemIcon:MovieClip;
      
      public function FilterItem()
      {
         super();
         _tooltipRendererName = "SrvProfileTipRenderer";
         buttonMode = true;
         addEventListener(MouseEvent.CLICK,this.onClick);
         addEventListener(MouseEvent.DOUBLE_CLICK,this.onClick);
      }
      
      override protected function initialize() : void
      {
         super.initialize();
         enabled = false;
         tabEnabled = true;
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
         _selectable = param1;
      }
      
      override public function get selected() : Boolean
      {
         return _selected;
      }
      
      override public function set selected(param1:Boolean) : void
      {
         var _loc2_:Boolean;
         if(_selected == param1)
         {
            return;
         }
         _selected = param1;
         if(_data)
         {
            _data.selected = _selected;
         }
         if(this.enabled)
         {
            if(!_focused)
            {
               setState("toggle");
            }
            else if(_pressedByKeyboard && _focusIndicator != null)
            {
               setState("kb_selecting");
            }
            else
            {
               setState("selecting");
            }
            if(owner)
            {
               _loc2_ = _selected && owner != null && checkOwnerFocused();
               setState(_loc2_ && _focusIndicator == null ? "selecting" : "toggle");
               displayFocus = _loc2_;
            }
         }
         else
         {
            setState("disabled");
         }
         validateNow();
         dispatchEvent(new Event(Event.SELECT));
      }
      
      override public function get enabled() : Boolean
      {
         return super.enabled;
      }
      
      override public function set enabled(param1:Boolean) : void
      {
         var _loc2_:String;
         super.enabled = param1;
         mouseChildren = false;
         if(super.enabled)
         {
            _loc2_ = _displayFocus || _focused ? "over" : "up";
         }
         else
         {
            _loc2_ = "disabled";
         }
         setState(_loc2_);
      }
      
      public function setListData(param1:ListData) : void
      {
         index = param1.index;
      }
      
      override protected function updateAfterStateChange() : void
      {
         super.updateAfterStateChange();
         if(this.itemIcon)
         {
            if(_data)
            {
               this.itemIcon.visible = true;
               this.itemIcon.gotoAndStop(300 + _data.icon);
            }
            else
            {
               this.itemIcon.gotoAndStop(1);
               this.itemIcon.visible = false;
            }
         }
      }
      
      public function setData(param1:Object) : void
      {
         focusable = false;
         mouseChildren = false;
         _data = param1;
         if(_data)
         {
            selected = _data.selected;
            if(_data.enabled != undefined)
            {
               enabled = _data.enabled;
            }
         }
         invalidateData();
         _focused = 0;
      }
      
      override protected function updateText() : void
      {
         var _loc1_:String;
         if(_data)
         {
            _loc1_ = _data.tip;
            if(textField)
            {
               textField.text = _loc1_;
            }
            _data.name = _loc1_;
            label = _loc1_;
         }
         else
         {
            selected = true;
            label = "";
            if(textField)
            {
               textField.text = " ";
            }
         }
         if(this.itemIcon)
         {
            if(_data)
            {
               this.itemIcon.visible = true;
               this.itemIcon.gotoAndStop(300 + _data.icon);
            }
            else
            {
               this.itemIcon.gotoAndStop(1);
               this.itemIcon.visible = false;
            }
         }
      }
      
      protected function onClick(param1:MouseEventEx) : void
      {
         if(owner && param1.buttonIdx == 0 && !MainScene.isShift)
         {
            (owner as FilterList).deselectAll();
            FilterList(owner).selectedIndex = this.index;
         }
         else
         {
            param1.buttonIdx = 0;
            super.handleMousePress(param1);
         }
      }
      
      override protected function configUI() : void
      {
         super.configUI();
         focusable = false;
         preventAutosizing = true;
         tabEnabled = _focusable && this.enabled && tabEnabled;
         tabChildren = false;
         focusTarget = owner;
         _focusable = tabEnabled = tabChildren = false;
         constraintsDisabled = true;
      }
   }
}

