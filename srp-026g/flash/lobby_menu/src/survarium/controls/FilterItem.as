package survarium.controls
{
   import flash.display.Bitmap;
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

      // Skhid: the attachment kinds' icons (ShopFilters 21 scopes, 22 muzzle devices, 23 grips): bitmaps
      // of the tree (flash/lobby_menu/art) on itemIcon's empty frames, centred where the stock icons
      // stand (34.75, 12), under the button's own colour states as they are.
      private static const DRAWN:Object = {21:FilterIconScope,22:FilterIconMuzzle,23:FilterIconGrip};

      private var _drawn:Bitmap;

      private var _drawnIcon:int = -1;
      
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
         _selectable = value;
      }
      
      override public function get selected() : Boolean
      {
         return _selected;
      }
      
      override public function set selected(value:Boolean) : void
      {
         var showFocus:Boolean;
         if(_selected == value)
         {
            return;
         }
         _selected = value;
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
               showFocus = _selected && owner != null && checkOwnerFocused();
               setState(showFocus && _focusIndicator == null ? "selecting" : "toggle");
               displayFocus = showFocus;
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
      
      override public function set enabled(value:Boolean) : void
      {
         var newState:String;
         super.enabled = value;
         mouseChildren = false;
         if(super.enabled)
         {
            newState = _displayFocus || _focused ? "over" : "up";
         }
         else
         {
            newState = "disabled";
         }
         setState(newState);
      }
      
      public function setListData(listData:ListData) : void
      {
         index = listData.index;
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
               this.showDrawn(int(_data.icon));
            }
            else
            {
               this.itemIcon.gotoAndStop(1);
               this.itemIcon.visible = false;
               this.showDrawn(-1);
            }
         }
      }
      
      private function showDrawn(icon:int) : void
      {
         if(icon == this._drawnIcon)
         {
            return;
         }
         this._drawnIcon = icon;
         if(this._drawn && this._drawn.parent)
         {
            this._drawn.parent.removeChild(this._drawn);
         }
         this._drawn = null;
         var art:Class = DRAWN[icon] as Class;
         if(!art)
         {
            return;
         }
         this._drawn = new Bitmap(new art(),"auto",true);
         this._drawn.x = Math.round(34.75 - (this._drawn.width - 1) / 2);
         this._drawn.y = Math.round(12 - (this._drawn.height - 1) / 2);
         this.itemIcon.addChild(this._drawn);
      }

      public function setData(newData:Object) : void
      {
         focusable = false;
         mouseChildren = false;
         _data = newData;
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
         var tipText:String;
         if(_data)
         {
            tipText = _data.tip;
            if(textField)
            {
               textField.text = tipText;
            }
            _data.name = tipText;
            label = tipText;
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
               this.showDrawn(int(_data.icon));
            }
            else
            {
               this.itemIcon.gotoAndStop(1);
               this.itemIcon.visible = false;
               this.showDrawn(-1);
            }
         }
      }
      
      protected function onClick(event:MouseEventEx) : void
      {
         if(owner && event.buttonIdx == MouseEventEx.LEFT_BUTTON && !MainScene.isShift)
         {
            (owner as FilterList).deselectAll();
            FilterList(owner).selectedIndex = this.index;
         }
         else
         {
            event.buttonIdx = MouseEventEx.LEFT_BUTTON;
            super.handleMousePress(event);
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

