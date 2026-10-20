package survarium.controls
{
   import flash.display.MovieClip;
   import flash.events.Event;
   import scaleform.clik.constants.InvalidationType;
   import scaleform.clik.controls.Button;
   import scaleform.clik.controls.ButtonBar;
   import scaleform.clik.events.ButtonBarEvent;
   import scaleform.clik.events.ButtonEvent;
   import scaleform.clik.events.IndexEvent;
   
   public class ButtonBarBottom extends ButtonBar
   {
      
      public var btn_prev:Button;
      
      public var btn_next:Button;
      
      public var buttons_background:MovieClip;
      
      protected var _toRemove:int = -1;
      
      private var _startIndex:int = 0;
      
      public function ButtonBarBottom()
      {
         super();
      }
      
      override public function set selectedIndex(param1:int) : void
      {
         if(param1 == _selectedIndex)
         {
            return;
         }
         var _loc2_:int = this._startIndex;
         if(param1 < this._startIndex && param1 != -1)
         {
            _startIndex = param1;
         }
         if(param1 > this._startIndex + _renderers.length - 1 && param1 != _dataProvider.length)
         {
            trace("hyer");
            _startIndex = param1 - _renderers.length + 1;
         }
         if(_loc2_ != this._startIndex)
         {
            this.updateRenderers();
         }
         var _loc3_:int = _selectedIndex;
         var _loc4_:Button = _renderers[_loc3_ - this._startIndex] as Button;
         if(_loc4_)
         {
            _loc4_.selected = false;
         }
         _selectedIndex = param1;
         _loc4_ = _renderers[_selectedIndex - this._startIndex] as Button;
         if(_loc4_)
         {
            _loc4_.selected = true;
         }
         dispatchEvent(new IndexEvent(IndexEvent.INDEX_CHANGE,true,true,_selectedIndex,_loc3_,_dataProvider[_selectedIndex]));
      }
      
      public function removeRenderer(param1:Button) : void
      {
         var _loc2_:uint;
         while(_loc2_ < _renderers.length)
         {
            if(_renderers[_loc2_] === param1)
            {
               _toRemove = _loc2_;
               break;
            }
            _loc2_++;
         }
      }
      
      override protected function configUI() : void
      {
         super.configUI();
         this.btn_prev.addEventListener(ButtonEvent.CLICK,this.arrowClick);
         this.btn_next.addEventListener(ButtonEvent.CLICK,this.arrowClick);
         this.btn_prev.visible = false;
         this.btn_next.visible = false;
      }
      
      protected function arrowClick(param1:ButtonEvent) : void
      {
         if(param1.target === this.btn_next)
         {
            _startIndex += 1;
         }
         else
         {
            _startIndex -= 1;
         }
         invalidateSettings();
      }
      
      protected function removeRendererAt(param1:int) : void
      {
         if(container.contains(_renderers[param1]))
         {
            container.removeChild(_renderers[param1]);
         }
         _renderers.splice(param1--,1);
      }
      
      protected function updateRenderersOnResize(param1:Event) : void
      {
         this.updateRenderers();
      }
      
      override protected function handleButtonGroupChange(param1:Event) : void
      {
         var _loc2_:ButtonEvent = param1 as ButtonEvent;
         if(_group.selectedIndex != selectedIndex)
         {
            selectedIndex = _group.selectedIndex + this._startIndex;
            dispatchEvent(new ButtonBarEvent(ButtonBarEvent.BUTTON_SELECT,false,true,_selectedIndex,param1.target as Button));
         }
      }
      
      override protected function updateRenderers() : void
      {
         var _loc1_:Number = 0;
         var _loc2_:Number = 0;
         var _loc3_:int = -1;
         if(this._toRemove != -1)
         {
            this.removeRendererAt(this._toRemove);
            _toRemove = -1;
         }
         if(_renderers[0] is _itemRendererClass)
         {
            while(_renderers.length > _dataProvider.length)
            {
               _loc5_ = _renderers.length - 1;
               if(container.contains(_renderers[_loc5_]))
               {
                  container.removeChild(_renderers[_loc5_]);
               }
               _renderers.splice(_loc5_--,1);
            }
         }
         else
         {
            while(container.numChildren > 0)
            {
               container.removeChildAt(0);
            }
            _renderers.length = 0;
         }
         var _loc4_:uint = 0;
         var _loc5_:int;
         var _loc6_:Button;
         var _loc7_:Boolean;
         var _loc8_:Number;
         var _loc9_:int;
         var _loc10_:Button;
         while(_loc4_ < _dataProvider.length && _loc3_ == -1)
         {
            _loc7_ = false;
            if(_loc4_ < _renderers.length)
            {
               _loc6_ = _renderers[_loc4_];
            }
            else
            {
               _loc6_ = new _itemRendererClass();
               _loc6_.addEventListener(Event.RESIZE,this.updateRenderersOnResize);
               setupRenderer(_loc6_,_loc4_);
               _loc7_ = true;
            }
            _loc6_.validateNow();
            if(_direction == DIRECTION_HORIZONTAL)
            {
               _loc8_ = _buttonWidth + _spacing + _loc1_ + this.buttons_background.width;
               if(_width > _buttonWidth + _spacing + _loc1_ || _maxFit && _maxSize > _loc8_)
               {
                  _loc6_.y = 0;
                  _loc6_.x = _loc1_;
                  _loc1_ += _loc6_.width + _spacing;
               }
               else
               {
                  _loc3_ = (_loc4_);
                  _loc6_ = null;
               }
            }
            else if(_height > _loc6_.height + _spacing + _loc2_)
            {
               _loc6_.x = 0;
               _loc6_.y = _loc2_;
               _loc2_ += _loc6_.height + _spacing;
            }
            else
            {
               _loc3_ = (_loc4_);
               _loc6_ = null;
            }
            if(_loc7_ && _loc6_ != null)
            {
               _loc6_.group = _group;
               container.addChild(_loc6_);
               _renderers.push(_loc6_);
            }
            _loc4_++;
         }
         if(isInvalid(InvalidationType.DATA))
         {
            if(_selectedIndex < this._startIndex && _selectedIndex != -1)
            {
               trace("lower");
               _startIndex = _selectedIndex;
            }
            if(_selectedIndex > this._startIndex + _renderers.length - 1 && _selectedIndex != _dataProvider.length)
            {
               trace("hyer");
               _startIndex = _selectedIndex - _renderers.length + 1;
            }
            if(_dataProvider.length <= _renderers.length)
            {
               _startIndex = 0;
            }
         }
         _loc4_ = 0;
         while(_loc4_ < _renderers.length)
         {
            _loc6_ = _renderers[_loc4_];
            populateRendererData(_loc6_,_loc4_ + this._startIndex);
            _loc4_++;
         }
         if(_loc3_ > -1)
         {
            _loc9_ = _renderers.length - 1;
            while(_loc9_ >= _loc3_)
            {
               _loc10_ = _renderers[_loc9_];
               if(_loc10_)
               {
                  if(container.contains(_loc10_))
                  {
                     container.removeChild(_loc10_);
                  }
                  _renderers.splice(_loc9_,1);
               }
               _loc9_--;
            }
            container.addChild(this.buttons_background);
            container.addChild(this.btn_prev);
            container.addChild(this.btn_next);
            this.buttons_background.visible = true;
            this.btn_prev.visible = true;
            this.btn_next.visible = true;
            this.btn_prev.enabled = this._startIndex > 0;
            this.btn_next.enabled = this._startIndex < _dataProvider.length - _loc3_;
            this.btn_prev.x = _loc1_ + 8;
            this.btn_next.x = this.btn_prev.x + this.btn_next.width + this.btn_prev.width - 2;
            this.buttons_background.x = this.btn_prev.x - 7;
         }
         else
         {
            this.btn_prev.x = 0;
            this.btn_next.x = this.btn_prev.x + this.btn_next.width + this.btn_prev.width;
            this.btn_prev.visible = false;
            this.btn_next.visible = false;
            this.btn_prev.enabled = false;
            this.btn_next.enabled = false;
            this.buttons_background.visible = false;
            this.buttons_background.x = 0;
         }
         selectedIndex = Math.min(_dataProvider.length - 1,_selectedIndex);
         dispatchEvent(new Event(Event.RESIZE));
      }
   }
}

