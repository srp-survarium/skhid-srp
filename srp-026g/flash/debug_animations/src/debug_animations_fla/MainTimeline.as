package debug_animations_fla
{
   import adobe.utils.*;
   import flash.accessibility.*;
   import flash.desktop.*;
   import flash.display.*;
   import flash.errors.*;
   import flash.events.*;
   import flash.external.*;
   import flash.filters.*;
   import flash.geom.*;
   import flash.globalization.*;
   import flash.media.*;
   import flash.net.*;
   import flash.net.drm.*;
   import flash.printing.*;
   import flash.profiler.*;
   import flash.sampler.*;
   import flash.sensors.*;
   import flash.system.*;
   import flash.text.*;
   import flash.text.engine.*;
   import flash.text.ime.*;
   import flash.ui.*;
   import flash.utils.*;
   import flash.xml.*;
   import survarium.debug.Row;
   
   public dynamic class MainTimeline extends MovieClip
   {
      
      public var names:Array;
      
      public var dataP:Array;
      
      public var _ofsetX:Number;
      
      public var _ofsetY:Number;
      
      public var _name_ofsetX:Number;
      
      public var rowH:int;
      
      public var _sx:int;
      
      public var _sy:int;
      
      public var columns:Vector.<Rower>;
      
      public var columnsNames:Array;
      
      public var columnsn:uint;
      
      public var content:MovieClip;
      
      public function MainTimeline()
      {
         super();
         addFrameScript(0,this.frame1);
      }
      
      public function set vertival_offset(value:Number) : *
      {
         _ofsetX = value;
         this.setOffset(this._ofsetX,this._ofsetY);
      }
      
      public function set horizontal_offset(value:Number) : *
      {
         _ofsetY = value;
         this.setOffset(this._ofsetX,this._ofsetY);
      }
      
      public function getHeight() : int
      {
         return int(this.content.height);
      }
      
      public function setOffset(offsetX:*, offsetY:*) : *
      {
         var columnX:* = this._sx;
         _ofsetX = offsetX;
         _ofsetY = offsetY;
         var i:int = 0;
         var column:Row;
         var header:*;
         while(i < this.columns.length)
         {
            column = this.columns[i];
            header = this.columnsNames[i];
            column.y = this._sy + 25;
            column.x = columnX;
            column.rowHeight = this.rowH + this._ofsetY;
            column.updateElemetns();
            header.y = this._sy + 5;
            header.x = columnX + this._name_ofsetX;
            if(column.width)
            {
               columnX += column.my_width + this._ofsetX;
            }
            i++;
         }
      }
      
      public function setShift(shiftX:int, shiftY:int = 0) : *
      {
         _sx = shiftX;
         _sy = shiftY;
         this.setOffset(this._ofsetX,this._ofsetY);
      }
      
      public function setProps(props:Array, reserved:int = 0) : *
      {
         var columnX:* = this._sx;
         var i:int = 0;
         var format:TextFormat;
         var column:Rower;
         var header:TextField;
         while(i < props.length)
         {
            if(i == this.columns.length || !this.columns[i])
            {
               column = new Rower();
               column.mouseChildren = false;
               column.mouseEnabled = false;
               this.columns.push(column);
            }
            else
            {
               column = this.columns[i];
            }
            column.rowHeight = this.rowH + this._ofsetY;
            column.my_width = props[i].width;
            column.columns = 1;
            format = new TextFormat();
            format.font = "tahoma";
            if(props[i].textAlign == undefined)
            {
               props[i].textAlign = 0;
            }
            if(props[i].textAlign == 0)
            {
               format.align = "left";
            }
            else if(props[i].textAlign == 1)
            {
               format.align = "center";
            }
            else
            {
               format.align = "right";
            }
            if(!this.columnsNames[i])
            {
               header = new TextField();
               header.textColor = 16777215;
               this.columnsNames[i] = header;
               header.selectable = false;
               header.filters = [new DropShadowFilter(0,45,0,1,4,4,1.5)];
            }
            else
            {
               header = this.columnsNames[i];
            }
            if(props[i].name)
            {
               header.text = props[i].name;
            }
            else
            {
               header.text = this.names[i];
            }
            header.width = props[i].width - this._name_ofsetX;
            header.setTextFormat(format);
            columnX += props[i].width + this._ofsetX;
            this.content.addChild(column);
            this.content.addChild(header);
            i++;
         }
         while(i < this.columns.length)
         {
            column = this.columns[i];
            header = this.columnsNames[i];
            this.content.removeChild(column);
            this.content.removeChild(header);
            i++;
         }
         this.setOffset(this._ofsetX,this._ofsetY);
      }
      
      public function setData(rows:Array) : void
      {
         var i:int;
         var row:Array;
         var j:int;
         i = 0;
         while(i < this.columns.length)
         {
            this.columns[i].length = rows.length;
            i++;
         }
         i = 0;
         while(i < rows.length)
         {
            row = rows[i];
            j = 0;
            while(j < row.length)
            {
               this.columns[j].elements[i].setData(rows[i][j]);
               j++;
            }
            i++;
         }
      }
      
      public function onStageResize(event:Event = null) : *
      {
         this.content.x = 0;
         this.content.y = 0;
      }
      
      internal function frame1() : *
      {
         names = ["Object name","Synchronization group\t","Additivity layer","Bones mask","Animation name","Weight","Time","scale","Time","Interval id","Interval time"];
         stage.scaleMode = StageScaleMode.NO_SCALE;
         dataP = [{
            "width":80,
            "textAlign":1
         },{"width":100},{"width":50},{"width":20},{"width":20},{"width":100},{"width":100},{"width":40},{"width":25},{"width":80}];
         _ofsetX = 2;
         _ofsetY = 2;
         _name_ofsetX = 2;
         rowH = 20;
         _sx = 10;
         _sy = 10;
         columns = new Vector.<Rower>();
         columnsNames = new Array();
         columnsn = 5;
         content = new MovieClip();
         addChild(this.content);
         this.setProps(this.dataP);
         dataP = [{
            "width":20,
            "textAlign":1
         },{"width":40},{"width":40},{"width":20},{"width":20},{"width":100},{"width":100},{"width":40},{"width":25},{"width":80}];
         this.setProps(this.dataP);
         stage.scaleMode = "noScale";
         stage.align = "TL";
         stage.addEventListener(Event.RESIZE,this.onStageResize);
      }
   }
}

